//! O que cada comando do app faz. O Tauri (`app/src-tauri`) e a ponte de teste
//! (`ward-ponte-teste`) chamam só isto, então os dois têm o mesmo conjunto de comandos
//! e as mesmas respostas. Nenhuma resposta carrega segredo (S1, S11, D4).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use ward_core::entrada::{Rascunho, RascunhoAnexo, Segredo as SegredoCore, TipoEntrada};
use ward_core::protocolo::Alvo;

use crate::ErroApp;
use crate::anexos::{Anexos, Base, EdicaoDosAnexos, Escolha, Seletor, SemSeletor};
use crate::aprovacao::{self, Aprovacoes, PedidoExibido, ResultadoAprovacao};
use crate::area_transferencia::{AreaTransferencia, Copiador};
use crate::backup_externo::{self, BackupExternoVisto, EscolhaDoPendrive, PendriveVisto};
use crate::chave::{self, CAMPOS_CARREGAVEIS, Carregamento};
use crate::cliente;
use crate::comandos::{Referencia, validar_nome_entrada};
use crate::configuracoes::{Configuracoes, ResultadoTroca, TrocaPin, TrocaSenha, minutos_validos};
use crate::credenciais::{DetalheCredencial, ItemCredencial};
use crate::daemon::{Daemon, EstadoCofre};
use crate::destrave::{InfoPedido, PedidoPendente, SenhaMestra};
use crate::formulario::{self, ErroCampo, Falta, Fontes, Formulario};
use crate::gerador::{Gerada, Gerador, Opcoes, Revelada};
use crate::nivel::{NivelMudado, Pin, nivel_de_nome};
use crate::primeiro_uso::NovoCofre;
use crate::restaurar::{self, Restauracao, Restaurado, VersaoVista};
use crate::segredo::Segredo;
use crate::terminal::{SemTerminal, Terminal, entrada_para_argv};

/// Campos do formulário que aceitam "Usar esta senha" (o `Campo secreto` com gerar no
/// design é só o da senha).
const CAMPOS_GERAVEIS: &[&str] = &["senha"];

/// Resposta do `salvar_credencial`: o nome salvo, ou os erros por campo. Sem valor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResultadoSalvar {
    pub salvo: Option<String>,
    pub erros: Vec<ErroCampo>,
}

/// Resposta do `criar_cofre`: criado, ou os erros por campo. Sem valor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResultadoCriar {
    pub criado: bool,
    pub erros: Vec<ErroCampo>,
}

/// Código TOTP para a interface (o código é mostrado, a semente nunca).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodigoTotp {
    pub codigo: String,
    pub restante: u64,
}

pub struct Servico<D: Daemon, A: AreaTransferencia> {
    daemon: Mutex<D>,
    copiador: Copiador<A>,
    /// Senha avulsa da tela `Gerador de senha` (Fase 37), só em memória.
    gerador: Mutex<Gerador>,
    /// Senhas que o gerador do formulário passou a um campo ("Usar esta senha"), só em
    /// memória, até salvar, cancelar ou sair.
    usadas: Mutex<BTreeMap<String, Segredo>>,
    /// Chaves privadas lidas pelo "Carregar de arquivo" (Fase 53a), por campo, só em
    /// memória, até salvar, cancelar ou sair. O conteúdo nunca vai à interface.
    carregadas: Mutex<BTreeMap<String, Segredo>>,
    /// Anexos escolhidos no formulário aberto (Fase 34), só em memória, até salvar,
    /// cancelar ou sair. O conteúdo nunca vai à interface.
    anexos: Mutex<Anexos>,
    /// Quem escolhe o arquivo do anexo (diálogo do GTK no app; pasta na ponte).
    seletor: Box<dyn Seletor>,
    /// Quem abre o terminal do "Conectar" (D3).
    terminal: Box<dyn Terminal>,
    /// Os `pedido_de_aprovacao` que esperam a janela (Fase 36, D65 a).
    aprovacoes: Arc<Aprovacoes>,
    /// O `pedido_de_destrave` do agente que está esperando (Fase 31, D65 a).
    pedido: Arc<PedidoPendente>,
}

/// O estado do backup externo: a configuração e, com um UUID escolhido, se ele está
/// entre os pendrives vistos. O udisks2 sem resposta não é erro aqui: o cartão diz que
/// não deu para ver.
fn backup_externo_de<D: Daemon + ?Sized>(d: &mut D) -> Result<BackupExternoVisto, ErroApp> {
    let (ligado, uuid) = cliente::backup_externo(d)?;
    let (pendrive, pendrives_visiveis) = match &uuid {
        None => (None, true),
        Some(u) => match cliente::pendrives(d) {
            Ok(lista) => (
                backup_externo::achar(&lista, u).map(PendriveVisto::de),
                true,
            ),
            Err(ErroApp::PendrivesIndisponiveis) => (None, false),
            Err(e) => return Err(e),
        },
    };
    Ok(BackupExternoVisto {
        ligado,
        uuid,
        pendrive,
        pendrives_visiveis,
    })
}

/// Valores que o formulário pede sem passar pela interface.
struct FontesDoServico<'a, D: Daemon> {
    daemon: &'a mut D,
    entrada: Option<&'a str>,
    usadas: &'a BTreeMap<String, Segredo>,
    carregadas: &'a BTreeMap<String, Segredo>,
}

impl<D: Daemon> Fontes for FontesDoServico<'_, D> {
    fn guardado(&mut self, alvo: Alvo) -> Result<SegredoCore, ErroApp> {
        let entrada = self.entrada.ok_or(ErroApp::PedidoInvalido)?;
        let s = cliente::revelar(self.daemon, entrada, alvo)?;
        Ok(SegredoCore::de_bytes(s.expor().to_vec()))
    }

    fn gerada(&mut self, campo: &str) -> Result<SegredoCore, ErroApp> {
        self.usadas
            .get(campo)
            .map(|s| SegredoCore::de_bytes(s.expor().to_vec()))
            .ok_or(ErroApp::SemSenhaGerada)
    }

    fn carregada(&mut self, campo: &str) -> Result<SegredoCore, ErroApp> {
        self.carregadas
            .get(campo)
            .map(|s| SegredoCore::de_bytes(s.expor().to_vec()))
            .ok_or(ErroApp::PedidoInvalido)
    }
}

impl<D: Daemon, A: AreaTransferencia> Servico<D, A> {
    pub fn novo(daemon: D, copiador: Copiador<A>) -> Self {
        Servico {
            daemon: Mutex::new(daemon),
            copiador,
            gerador: Mutex::new(Gerador::default()),
            usadas: Mutex::new(BTreeMap::new()),
            carregadas: Mutex::new(BTreeMap::new()),
            anexos: Mutex::new(Anexos::default()),
            seletor: Box::new(SemSeletor),
            terminal: Box::new(SemTerminal),
            aprovacoes: Arc::new(Aprovacoes::default()),
            pedido: Arc::new(PedidoPendente::default()),
        }
    }

    /// O pedido de destrave que a vigia da fila de avisos marca
    /// ([`crate::destrave::vigiar`]).
    pub fn pedido_de_destrave(&self) -> Arc<PedidoPendente> {
        Arc::clone(&self.pedido)
    }

    /// O pedido do agente que espera o destrave, se o cofre está travado e o pedido
    /// ainda está no prazo. Sem entrada nem comando (o aviso não os traz).
    pub fn info_do_pedido(&self, estado: EstadoCofre) -> Option<InfoPedido> {
        match estado {
            EstadoCofre::Travado => self.pedido.info(),
            _ => None,
        }
    }

    /// `destravar { senha }` (Fase 31): a senha chega já conferida no formato, num
    /// [`SenhaMestra`] que zera, e só vai ao daemon. A espera de D20 é do daemon (antes de
    /// olhar a senha), e a resposta não diz quantas tentativas houve. Destravou: o pedido
    /// do agente segue sozinho no daemon e sai da tela.
    pub fn destravar(&self, senha: SenhaMestra) -> Result<(), ErroApp> {
        let r = cliente::destravar(&mut *self.daemon()?, &senha);
        drop(senha);
        r?;
        self.pedido.limpar();
        Ok(())
    }

    /// Os pedidos de aprovação que a vigia da fila de avisos preenche
    /// ([`crate::aprovacao::vigiar`]).
    pub fn aprovacoes(&self) -> Arc<Aprovacoes> {
        Arc::clone(&self.aprovacoes)
    }

    /// `pedido_de_aprovacao` (Fase 36): o pedido da vez como a janela o mostra, com todo
    /// texto do agente neutralizado (D51), ou `None`. Os tipos das entradas vêm do
    /// detalhe (uma vez por pedido; sem ele, a tela mostra o ícone genérico).
    pub fn pedido_de_aprovacao(&self) -> Result<Option<PedidoExibido>, ErroApp> {
        let Some(mut atual) = self.aprovacoes.atual() else {
            return Ok(None);
        };
        if atual.tipos.is_none() && !atual.bloqueado {
            let mut d = self.daemon()?;
            let tipos: Vec<Option<&'static str>> = atual
                .pedido
                .entradas
                .iter()
                .map(|n| cliente::detalhe(&mut *d, n).ok().map(|x| x.tipo.nome()))
                .collect();
            drop(d);
            self.aprovacoes
                .guardar_tipos(atual.pedido.id, tipos.clone());
            atual.tipos = Some(tipos);
        }
        Ok(Some(aprovacao::exibir(&atual)))
    }

    /// `responder_aprovacao { id, decisao, pin }` (Fase 36, S9, S10). Só um pedido que o
    /// daemon mandou a este app e que ainda está aqui; "Negar" sem PIN, os três
    /// "Permitir" com PIN (já conferido no formato, num [`Pin`] que zera).
    /// - PIN errado: o pedido continua pendente; se o cofre travou (5º erro), o daemon
    ///   já negou e a janela mostra `bloqueado`.
    /// - "Negar" num pedido `bloqueado` só o tira da janela (o daemon já negou).
    pub fn responder_aprovacao(
        &self,
        id: u64,
        decisao: &str,
        pin: Option<Pin>,
    ) -> Result<ResultadoAprovacao, ErroApp> {
        let decisao = aprovacao::decisao_de_nome(decisao)?;
        if decisao.pede_pin() != pin.is_some() {
            return Err(ErroApp::PedidoInvalido);
        }
        let resultado = match self.aprovacoes.situacao(id) {
            None => return Err(ErroApp::PedidoInvalido),
            Some(true) => {
                if decisao.pede_pin() {
                    return Err(ErroApp::Travado);
                }
                self.aprovacoes.remover(id);
                "respondido"
            }
            Some(false) => {
                // Fase 44c: a confirmação da origem só se decide por "Negar" ou
                // "Confirmar"; os outros pedidos nunca por "Confirmar" (o daemon confere
                // de novo).
                let confirmacao = self.aprovacoes.confirmacao(id).unwrap_or(false);
                if decisao != ward_core::protocolo::Decisao::Negar
                    && confirmacao != (decisao == ward_core::protocolo::Decisao::Confirmar)
                {
                    return Err(ErroApp::PedidoInvalido);
                }
                let r =
                    cliente::responder_aprovacao(&mut *self.daemon()?, id, decisao, pin.as_ref());
                drop(pin);
                match r? {
                    cliente::Respondida::Ok => {
                        self.aprovacoes.remover(id);
                        "respondido"
                    }
                    cliente::Respondida::Expirado => {
                        self.aprovacoes.remover(id);
                        "expirado"
                    }
                    cliente::Respondida::PinIncorreto => {
                        if self.estado_do_cofre()? == EstadoCofre::Destravado {
                            "pin-incorreto"
                        } else {
                            // O 5º erro travou o cofre: o daemon negou o pedido (S9).
                            self.aprovacoes.marcar_bloqueado(id);
                            "bloqueado"
                        }
                    }
                }
            }
        };
        Ok(ResultadoAprovacao {
            resultado,
            ha_outro: self.aprovacoes.ha_pedido(),
        })
    }

    /// Fechar a janela nega (S10): todo pedido pendente aqui recebe "Negar". Os
    /// bloqueados só saem (o daemon já negou). Falha ao falar com o daemon não abre
    /// nada: sem resposta, o pedido expira e nega no daemon.
    pub fn negar_aprovacoes_pendentes(&self) {
        for id in self.aprovacoes.pendentes() {
            if let Ok(mut d) = self.daemon() {
                let _ = cliente::responder_aprovacao(
                    &mut *d,
                    id,
                    ward_core::protocolo::Decisao::Negar,
                    None,
                );
            }
            self.aprovacoes.remover(id);
        }
        self.aprovacoes.esvaziar();
    }

    /// Troca o seletor de arquivo (sem ele, `escolher_anexo` responde o erro).
    pub fn com_seletor(mut self, seletor: impl Seletor + 'static) -> Self {
        self.seletor = Box::new(seletor);
        self
    }

    /// Troca o terminal do "Conectar" (sem ele, `conectar` responde o erro).
    pub fn com_terminal(mut self, terminal: impl Terminal + 'static) -> Self {
        self.terminal = Box::new(terminal);
        self
    }

    fn anexos_travados(&self) -> MutexGuard<'_, Anexos> {
        // Pânico com o lock na mão: os anexos escolhidos são descartados.
        self.anexos.lock().unwrap_or_else(|e| {
            let mut a = e.into_inner();
            a.limpar();
            a
        })
    }

    /// `escolher_anexo`: o seletor do lado Rust escolhe o arquivo e ele é lido aqui
    /// (limites de D29/D32). A resposta traz id, nome e tamanho, ou o aviso; nunca o
    /// conteúdo (S11, V5.12).
    ///
    /// Fase 39b: na edição (`edicao`), os anexos guardados que a tela mantém contam nos
    /// limites; os nomes são conferidos no detalhe da entrada antes de abrir o seletor
    /// (nome desconhecido ou repetido é pedido inválido).
    pub fn escolher_anexo(&self, edicao: Option<&EdicaoDosAnexos>) -> Result<Escolha, ErroApp> {
        let base = match edicao {
            None => Base::default(),
            Some(ed) => self.base_da_edicao(ed)?,
        };
        match self.seletor.escolher()? {
            None => Ok(Escolha::Cancelado),
            Some(caminho) => Ok(self.anexos_travados().anexar_sobre(&caminho, base)),
        }
    }

    /// Soma e quantidade dos anexos guardados de `ed.entrada` que a tela mantém.
    fn base_da_edicao(&self, ed: &EdicaoDosAnexos) -> Result<Base, ErroApp> {
        validar_nome_entrada(&ed.entrada)?;
        if ed.mantidos.len() > ward_core::limites::MAX_ANEXOS {
            return Err(ErroApp::PedidoInvalido);
        }
        let det = cliente::detalhe(&mut *self.daemon()?, &ed.entrada)?;
        let mut base = Base::default();
        for (i, n) in ed.mantidos.iter().enumerate() {
            let guardado = det.anexos.iter().find(|a| &a.nome == n);
            match guardado {
                Some(a) if !ed.mantidos[..i].contains(n) => {
                    let t = usize::try_from(a.tamanho).map_err(|_| ErroApp::RespostaInvalida)?;
                    base.soma = base.soma.saturating_add(t);
                    base.quantidade += 1;
                }
                _ => return Err(ErroApp::PedidoInvalido),
            }
        }
        Ok(base)
    }

    /// `carregar_chave { campo }` (Fase 53a): "Carregar de arquivo" do campo "Chave
    /// privada". O campo é conferido antes de abrir o seletor; o arquivo é lido e
    /// conferido aqui ([`chave::ler_chave`]: sem link, só arquivo comum, até 64 KiB, só
    /// chave OpenSSH/PEM) e guardado até salvar, cancelar ou sair. A resposta traz só o
    /// tamanho, ou o aviso; nunca o conteúdo nem o caminho (S2, S11). Um aviso ou um
    /// cancelamento não apagam a chave carregada antes.
    pub fn carregar_chave(&self, campo: &str) -> Result<Carregamento, ErroApp> {
        let campo = CAMPOS_CARREGAVEIS
            .iter()
            .find(|c| **c == campo)
            .ok_or(ErroApp::PedidoInvalido)?;
        let Some(caminho) = self.seletor.escolher_chave()? else {
            return Ok(Carregamento::Cancelado);
        };
        match chave::ler_chave(&caminho) {
            Ok(s) => {
                let tamanho = s.len() as u64;
                self.carregadas_travadas().insert((*campo).to_owned(), s);
                Ok(Carregamento::Carregada { tamanho })
            }
            Err(motivo) => Ok(Carregamento::Aviso { motivo }),
        }
    }

    fn carregadas_travadas(&self) -> MutexGuard<'_, BTreeMap<String, Segredo>> {
        // Pânico com o lock na mão: as chaves carregadas são descartadas.
        self.carregadas.lock().unwrap_or_else(|e| {
            let mut m = e.into_inner();
            m.clear();
            m
        })
    }

    /// `remover_anexo { id }`: tira um anexo escolhido e ainda não salvo.
    pub fn remover_anexo(&self, id: u64) -> Result<(), ErroApp> {
        self.anexos_travados().remover(id)
    }

    /// `conectar { entrada }` (D3): só `servidor`. O daemon emite o token de uso único e
    /// o terminal abre `ward ssh --token-fd <n> <entrada>` e o token vai pelo canal do fd
    /// (Fase 27a), nunca em argv. A resposta não traz o token.
    pub fn conectar(&self, entrada: &str) -> Result<(), ErroApp> {
        validar_nome_entrada(entrada)?;
        if !entrada_para_argv(entrada) {
            return Err(ErroApp::PedidoInvalido);
        }
        let token = {
            let mut d = self.daemon()?;
            if cliente::detalhe(&mut *d, entrada)?.tipo != TipoEntrada::Servidor {
                return Err(ErroApp::NaoEServidor);
            }
            cliente::emitir_token(&mut *d, entrada)?
        };
        self.terminal.abrir(entrada, &token)
    }

    fn daemon(&self) -> Result<MutexGuard<'_, D>, ErroApp> {
        self.daemon.lock().map_err(|_| ErroApp::DaemonIndisponivel)
    }

    /// `estado_do_cofre`.
    pub fn estado_do_cofre(&self) -> Result<EstadoCofre, ErroApp> {
        cliente::estado(&mut *self.daemon()?)
    }

    /// `criar_cofre` (primeiro uso, Fase 30): confere senha mestra, PIN e as duas
    /// confirmações aqui e só então pede ao daemon. Com erro de campo, nada vai ao
    /// daemon. A resposta não traz valor nenhum; o `novo` é zerado ao sair.
    pub fn criar_cofre(&self, novo: NovoCofre) -> Result<ResultadoCriar, ErroApp> {
        let erros = novo.erros();
        if !erros.is_empty() {
            return Ok(ResultadoCriar {
                criado: false,
                erros,
            });
        }
        cliente::criar_cofre(&mut *self.daemon()?, novo.senha.bytes(), novo.pin.bytes())?;
        Ok(ResultadoCriar {
            criado: true,
            erros: Vec::new(),
        })
    }

    /// `listar_credenciais`: só a `lista`, numa troca; a segunda linha de cada item
    /// (usuário, porta, motor) e o usuário da busca vêm do resumo do papel app (D65 c),
    /// sem um `detalhe` por entrada. Nada de segredo. Item sem resumo ou sem nível
    /// recusa a resposta inteira.
    pub fn listar_credenciais(&self) -> Result<Vec<ItemCredencial>, ErroApp> {
        let itens = cliente::lista(&mut *self.daemon()?)?;
        itens
            .iter()
            .map(|item| ItemCredencial::novo(item).ok_or(ErroApp::RespostaInvalida))
            .collect()
    }

    /// `detalhe_credencial`: o painel de uma entrada. O nome vem da interface e é
    /// validado antes de ir ao daemon.
    pub fn detalhe_credencial(&self, entrada: &str) -> Result<DetalheCredencial, ErroApp> {
        validar_nome_entrada(entrada)?;
        let d = cliente::detalhe(&mut *self.daemon()?, entrada)?;
        Ok(DetalheCredencial::novo(&d))
    }

    /// `copiar_campo`: o valor vai do daemon direto para a área de transferência. A
    /// resposta é só "deu certo" ou o erro; o valor não volta.
    ///
    /// O campo é procurado no `detalhe` da entrada: segredo do tipo ou extra secreto
    /// vão pelo `copiar` do protocolo; campo de texto ou extra comum, pelo valor que o
    /// daemon deu no detalhe (nunca pelo que a interface mandou).
    pub fn copiar_campo(&self, entrada: &str, campo: &str) -> Result<(), ErroApp> {
        let referencia = Referencia::nova(entrada, campo)?;
        let valor = {
            let mut d = self.daemon()?;
            let detalhe = cliente::detalhe(&mut *d, referencia.entrada())?;
            let campo = referencia.campo();
            if detalhe.segredos.iter().any(|s| s == campo) {
                cliente::copiar(&mut *d, referencia.entrada(), Alvo::Campo(campo.to_owned()))?
            } else if let Some((_, v)) = detalhe.campos.iter().find(|(k, _)| k == campo) {
                Segredo::novo(v.as_bytes().to_vec())
            } else if let Some(x) = detalhe.extras.iter().find(|x| x.nome == campo) {
                match &x.valor {
                    Some(v) => Segredo::novo(v.as_bytes().to_vec()),
                    None => cliente::copiar(
                        &mut *d,
                        referencia.entrada(),
                        Alvo::Extra(campo.to_owned()),
                    )?,
                }
            } else {
                return Err(ErroApp::CampoInexistente);
            }
        };
        self.copiador.copiar(valor)
    }

    /// `gerar_senha`: gera no `ward_core::gerador` e guarda aqui. A resposta tem só
    /// tamanho, força e símbolos; o valor não volta.
    pub fn gerar_senha(&self, opcoes: Opcoes) -> Result<Gerada, ErroApp> {
        self.gerador_travado()?.gerar(opcoes)
    }

    /// `copiar_senha_gerada`: mesmo caminho do `copiar_campo` (D4, S12). Resposta `()`.
    pub fn copiar_senha_gerada(&self) -> Result<(), ErroApp> {
        let valor = self.gerador_travado()?.para_copiar()?;
        self.copiador.copiar(valor)
    }

    /// `revelar_senha_gerada`: o único comando que leva o valor à interface, no clique
    /// em "revelar" (S11).
    pub fn revelar_senha_gerada(&self) -> Result<Revelada, ErroApp> {
        self.gerador_travado()?.revelar()
    }

    /// `descartar_senha_gerada`: ao sair da tela, a senha zera.
    pub fn descartar_senha_gerada(&self) -> Result<(), ErroApp> {
        self.gerador_travado()?.descartar();
        Ok(())
    }

    /// `revelar_campo`: o valor de um segredo da entrada (campo do tipo ou extra
    /// secreto), só no clique em "revelar" (S11). Campo de texto não passa por aqui.
    pub fn revelar_campo(&self, entrada: &str, campo: &str) -> Result<Revelada, ErroApp> {
        let referencia = Referencia::nova(entrada, campo)?;
        let mut d = self.daemon()?;
        let detalhe = cliente::detalhe(&mut *d, referencia.entrada())?;
        let campo = referencia.campo();
        let alvo = if detalhe.segredos.iter().any(|s| s == campo) {
            Alvo::Campo(campo.to_owned())
        } else if detalhe
            .extras
            .iter()
            .any(|x| x.nome == campo && x.valor.is_none())
        {
            Alvo::Extra(campo.to_owned())
        } else {
            return Err(ErroApp::CampoInexistente);
        };
        let valor = cliente::revelar(&mut *d, referencia.entrada(), alvo)?;
        Revelada::de_segredo(&valor).ok_or(ErroApp::Recusado)
    }

    /// `codigo_totp`: o código atual da entrada e os segundos até trocar.
    pub fn codigo_totp(&self, entrada: &str) -> Result<CodigoTotp, ErroApp> {
        validar_nome_entrada(entrada)?;
        let c = cliente::totp(&mut *self.daemon()?, entrada)?;
        Ok(CodigoTotp {
            codigo: c.codigo.as_str().to_owned(),
            restante: c.restante,
        })
    }

    /// `copiar_codigo_totp`: o código vai para a área de transferência (D4).
    pub fn copiar_codigo_totp(&self, entrada: &str) -> Result<(), ErroApp> {
        validar_nome_entrada(entrada)?;
        let c = cliente::totp(&mut *self.daemon()?, entrada)?;
        self.copiador
            .copiar(Segredo::novo(c.codigo.as_bytes().to_vec()))
    }

    /// `usar_senha_gerada { campo }`: a senha do gerador passa para o campo do
    /// formulário, no lado Rust. A interface recebe só o tamanho (para a máscara).
    pub fn usar_senha_gerada(&self, campo: &str) -> Result<usize, ErroApp> {
        let campo = CAMPOS_GERAVEIS
            .iter()
            .find(|c| **c == campo)
            .ok_or(ErroApp::PedidoInvalido)?;
        let senha = self.gerador_travado()?.para_copiar()?;
        let tamanho = senha.len();
        self.usadas_travadas().insert((*campo).to_owned(), senha);
        Ok(tamanho)
    }

    /// `descartar_formulario`: ao cancelar ou sair do formulário, zera as senhas usadas
    /// e a do gerador.
    pub fn descartar_formulario(&self) {
        self.usadas_travadas().clear();
        self.carregadas_travadas().clear();
        self.anexos_travados().limpar();
        if let Ok(mut g) = self.gerador_travado() {
            g.descartar();
        }
    }

    /// `salvar_credencial { anterior?, formulario }`: monta o rascunho no lado Rust
    /// (segredos mantidos e senha gerada não passam pela interface) e manda o
    /// `salvar_entrada`. A resposta é o nome salvo ou os erros por campo.
    pub fn salvar_credencial(
        &self,
        anterior: Option<&str>,
        mut f: Formulario,
    ) -> Result<ResultadoSalvar, ErroApp> {
        let tipo = formulario::tipo_do_formulario(&f.tipo).ok_or(ErroApp::PedidoInvalido)?;
        if let Some(a) = anterior {
            validar_nome_entrada(a)?;
        }
        let mut d = self.daemon()?;
        let editada = match anterior {
            Some(a) => {
                let det = cliente::detalhe(&mut *d, a)?;
                if det.tipo != tipo {
                    return Err(ErroApp::PedidoInvalido);
                }
                Some(det)
            }
            None => None,
        };
        // Fase 39b (V5.12): anexos mantidos só na edição, só os que a entrada tem e cada
        // um uma vez (a interface não é confiável). O conteúdo fica no daemon.
        let mantidos = std::mem::take(&mut f.anexos_mantidos);
        let guardados: &[ward_core::protocolo::AnexoDetalhe] =
            editada.as_ref().map_or(&[], |d| d.anexos.as_slice());
        for (i, n) in mantidos.iter().enumerate() {
            if mantidos[..i].contains(n) || !guardados.iter().any(|a| &a.nome == n) {
                return Err(ErroApp::PedidoInvalido);
            }
        }
        let extras_guardados: Vec<String> = editada
            .as_ref()
            .map(|det| {
                det.extras
                    .iter()
                    .filter(|x| x.valor.is_none())
                    .map(|x| x.nome.clone())
                    .collect()
            })
            .unwrap_or_default();
        let editando = editada
            .as_ref()
            .map(|det| (det.segredos.as_slice(), extras_guardados.as_slice()));
        let usadas = self.usadas_travadas();
        let carregadas = self.carregadas_travadas();
        let mut fontes = FontesDoServico {
            daemon: &mut *d,
            entrada: anterior,
            usadas: &usadas,
            carregadas: &carregadas,
        };
        let montado = match formulario::montar(&mut f, tipo, editando, &mut fontes) {
            Ok(m) => m,
            Err(Falta::App(e)) => return Err(e),
            Err(Falta::Campos(erros)) => {
                return Ok(ResultadoSalvar { salvo: None, erros });
            }
        };
        drop(usadas);
        drop(carregadas);
        // Os anexos pedidos, copiados: se o daemon recusar, continuam escolhidos para o
        // próximo salvar.
        let anexos: Vec<RascunhoAnexo> = self
            .anexos_travados()
            .escolhidos(&f.anexos)?
            .into_iter()
            .map(|p| RascunhoAnexo {
                nome: p.nome.clone(),
                conteudo: SegredoCore::de_bytes(p.conteudo.expor().to_vec()),
            })
            .collect();
        let nome = f.nome.clone();
        let rascunho = Rascunho {
            nome: f.nome,
            grupo: f.grupo,
            ambiente: f.ambiente,
            projeto: f.projeto,
            tags: f.tags,
            dados: montado.dados,
            extras: montado.extras,
            anexos,
        };
        if let Err(erros) = formulario::validar_local(&rascunho) {
            return Ok(ResultadoSalvar { salvo: None, erros });
        }
        match cliente::salvar(&mut *d, anterior.map(str::to_owned), rascunho, mantidos)? {
            cliente::Salvamento::Salvo => {
                drop(d);
                self.descartar_formulario();
                Ok(ResultadoSalvar {
                    salvo: Some(nome),
                    erros: Vec::new(),
                })
            }
            cliente::Salvamento::Problemas(p) => Ok(ResultadoSalvar {
                salvo: None,
                erros: formulario::erros_do_daemon(&p),
            }),
        }
    }

    /// `mudar_nivel { entrada, nivel, pin }` (Fase 35, S9): o nome e o nível vêm da
    /// interface e são conferidos aqui; o PIN já chega conferido no formato, num [`Pin`]
    /// que zera. A resposta é o nível novo, sem PIN. PIN errado é
    /// [`ErroApp::PinIncorreto`] (a tela consulta o estado depois: o 5º erro trava).
    pub fn mudar_nivel(
        &self,
        entrada: &str,
        nivel: &str,
        pin: Pin,
    ) -> Result<NivelMudado, ErroApp> {
        validar_nome_entrada(entrada)?;
        let nivel = nivel_de_nome(nivel)?;
        cliente::mudar_nivel(&mut *self.daemon()?, entrada, nivel, &pin)?;
        drop(pin);
        Ok(NivelMudado {
            entrada: entrada.to_owned(),
            nivel: nivel.nome(),
        })
    }

    /// `configuracoes` (Fase 38): tempo de trava e backup externo, sem hash nem UUID.
    pub fn configuracoes(&self) -> Result<Configuracoes, ErroApp> {
        cliente::configuracao(&mut *self.daemon()?)
    }

    /// `trocar_pin { dados }` (Fase 38, V5.22, S9): confere o formato dos três campos e a
    /// confirmação aqui; com erro de campo, nada vai ao daemon. O daemon confere o PIN
    /// atual (o 5.º erro seguido trava). A resposta não traz valor.
    pub fn trocar_pin(&self, troca: TrocaPin) -> Result<ResultadoTroca, ErroApp> {
        let erros = troca.erros();
        if !erros.is_empty() {
            return Ok(ResultadoTroca::com_erros(erros));
        }
        cliente::trocar_pin(
            &mut *self.daemon()?,
            troca.atual.bytes(),
            troca.novo.bytes(),
        )?;
        Ok(ResultadoTroca::trocado())
    }

    /// `trocar_senha_mestra { dados }` (Fase 38, V5.22): a nova segue a regra do primeiro
    /// uso; o daemon confere a atual. A resposta não traz valor.
    pub fn trocar_senha_mestra(&self, troca: TrocaSenha) -> Result<ResultadoTroca, ErroApp> {
        let erros = troca.erros();
        if !erros.is_empty() {
            return Ok(ResultadoTroca::com_erros(erros));
        }
        cliente::trocar_senha(
            &mut *self.daemon()?,
            troca.atual.bytes(),
            troca.nova.bytes(),
        )?;
        Ok(ResultadoTroca::trocado())
    }

    /// `definir_tempo_trava { minutos }` (Fase 38, D30, D45): fora de 1 a 1440 nem chega
    /// ao daemon. A resposta é a configuração lida de novo do daemon.
    pub fn definir_tempo_trava(&self, minutos: u64) -> Result<Configuracoes, ErroApp> {
        let minutos = minutos_validos(minutos)?;
        let mut d = self.daemon()?;
        cliente::tempo_trava(&mut *d, minutos)?;
        cliente::configuracao(&mut *d)
    }

    /// `backup_externo` (Fase 48, V7.9): ligado ou não, o UUID escolhido e, se ele
    /// estiver conectado, o pendrive (rótulo neutralizado, D51). Sem caminho de montagem.
    pub fn backup_externo(&self) -> Result<BackupExternoVisto, ErroApp> {
        backup_externo_de(&mut *self.daemon()?)
    }

    /// `listar_pendrives` (Fase 48): os pendrives que o daemon vê, para escolher.
    pub fn listar_pendrives(&self) -> Result<Vec<PendriveVisto>, ErroApp> {
        let lista = cliente::pendrives(&mut *self.daemon()?)?;
        Ok(lista.iter().map(PendriveVisto::de).collect())
    }

    /// `definir_backup_externo { ligado, uuid? }` (Fase 48, V7.9; D17, D30). Ligar: o UUID
    /// (formato de D45, conferido em [`EscolhaDoPendrive::nova`]) tem de ser de um pendrive que o
    /// daemon vê agora; senão nada vai ao `definir_backup_externo`. Vai o UUID como o
    /// daemon o deu. Desligar mantém o UUID guardado. A resposta é o estado lido de novo.
    pub fn definir_backup_externo(
        &self,
        ligado: bool,
        uuid: Option<&str>,
    ) -> Result<BackupExternoVisto, ErroApp> {
        let escolha = EscolhaDoPendrive::nova(ligado, uuid)?;
        let mut d = self.daemon()?;
        match escolha {
            EscolhaDoPendrive::Ligar(u) => {
                let lista = cliente::pendrives(&mut *d)?;
                let p = backup_externo::achar(&lista, &u).ok_or(ErroApp::PendriveNaoConectado)?;
                cliente::definir_backup_externo(&mut *d, true, Some(p.uuid.clone()))?;
            }
            EscolhaDoPendrive::Desligar => {
                let (_, atual) = cliente::backup_externo(&mut *d)?;
                cliente::definir_backup_externo(&mut *d, false, atual)?;
            }
        }
        backup_externo_de(&mut *d)
    }

    /// `listar_backups` (Fase 47, V7.7): as versões do backup local, da mais nova para a
    /// mais antiga, só com o nome e o instante (do nome). Nenhum conteúdo, tamanho ou
    /// caminho: o protocolo não traz.
    pub fn listar_backups(&self) -> Result<Vec<VersaoVista>, ErroApp> {
        let lista = cliente::backups(&mut *self.daemon()?)?;
        restaurar::versoes_de(&lista)
    }

    /// `restaurar_backup { versao, senha, pin }` (Fase 47, V7.7; D17, D39, S9). A versão
    /// tem de estar no formato e na lista que o daemon dá agora; senão nada vai ao
    /// `restaurar_backup` ([`ErroApp::BackupInexistente`]). A senha da versão e o PIN
    /// atual só vão no pedido e zeram ao sair. Depois da troca, o formulário aberto (do
    /// cofre de antes) é descartado e a resposta traz só quantas entradas a versão tem.
    pub fn restaurar_backup(&self, pedido: Restauracao) -> Result<Restaurado, ErroApp> {
        let Restauracao { versao, senha, pin } = pedido;
        restaurar::versao_pedida(&versao)?;
        let mut d = self.daemon()?;
        if !cliente::backups(&mut *d)?.iter().any(|b| b.nome == versao) {
            return Err(ErroApp::BackupInexistente);
        }
        let r = cliente::restaurar_backup(&mut *d, &versao, &senha, &pin);
        drop(senha);
        drop(pin);
        r?;
        let entradas = cliente::lista(&mut *d)?.len();
        drop(d);
        self.descartar_formulario();
        Ok(Restaurado {
            restaurado: true,
            entradas,
        })
    }

    fn usadas_travadas(&self) -> MutexGuard<'_, BTreeMap<String, Segredo>> {
        // Pânico com o lock na mão: as senhas usadas são descartadas.
        self.usadas.lock().unwrap_or_else(|e| {
            let mut m = e.into_inner();
            m.clear();
            m
        })
    }

    fn gerador_travado(&self) -> Result<std::sync::MutexGuard<'_, Gerador>, ErroApp> {
        // Um pânico com o lock na mão não deixa a senha presa: o estado é descartado.
        Ok(self.gerador.lock().unwrap_or_else(|e| {
            let mut g = e.into_inner();
            g.descartar();
            g
        }))
    }

    /// "Travar agora" (janela e bandeja, Fase 39): limpa a área de transferência (se
    /// ainda tem o valor copiado) e o que o formulário e o gerador guardavam **antes** de
    /// pedir a trava, e o pedido do agente que esperava o destrave sai da tela. Mesmo com
    /// o daemon fora, o que é local é limpo.
    pub fn travar(&self) -> Result<(), ErroApp> {
        self.ao_sair();
        let r = cliente::travar(&mut *self.daemon()?);
        self.ao_sair();
        r
    }

    /// Estado para a bandeja (Fase 39): só `estado`, que o daemon não conta como uso.
    pub fn estado_da_bandeja(&self) -> crate::bandeja::EstadoBandeja {
        crate::bandeja::EstadoBandeja::de(self.estado_do_cofre())
    }

    /// A bandeja viu o estado mudar (Fase 53d): se o cofre deixou de estar destravado
    /// (trava vinda de fora do app), zera o que o formulário guardava. Fase 39f (S12):
    /// e limpa a área de transferência na hora, se ela ainda tem o valor do Ward, como o
    /// "Travar agora" do app.
    pub fn na_passagem_da_bandeja(
        &self,
        antes: crate::bandeja::EstadoBandeja,
        depois: crate::bandeja::EstadoBandeja,
    ) {
        if crate::bandeja::descartar_na_passagem(antes, depois) {
            self.ao_sair();
        }
    }

    /// A vigia do app consulta o estado (Fase 39g): troca o último estado visto pelo de
    /// agora e, na passagem de destravado para outro, limpa a área de transferência e o
    /// formulário ([`Self::na_passagem_da_bandeja`]). Vale com ou sem bandeja: o
    /// `visto` é do app, não do menu. Devolve `(antes, agora)`.
    pub fn ver_passagem(
        &self,
        visto: &Mutex<crate::bandeja::EstadoBandeja>,
    ) -> (crate::bandeja::EstadoBandeja, crate::bandeja::EstadoBandeja) {
        let agora = self.estado_da_bandeja();
        let antes = {
            let mut v = visto.lock().unwrap_or_else(|e| e.into_inner());
            core::mem::replace(&mut *v, agora)
        };
        self.na_passagem_da_bandeja(antes, agora);
        (antes, agora)
    }

    /// Ao travar ou fechar o app: limpa a área se ainda tiver o valor copiado e zera a
    /// senha avulsa.
    pub fn ao_sair(&self) {
        self.copiador.limpar_agora();
        self.descartar_formulario();
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::daemon_falso::{DaemonFalso, SEGREDOS};
    use std::sync::Arc;
    use zeroize::Zeroizing;

    #[derive(Default)]
    struct Area(Mutex<Option<Vec<u8>>>);

    impl AreaTransferencia for Arc<Area> {
        fn escrever(&self, v: &[u8]) -> Result<(), ErroApp> {
            *self.0.lock().unwrap() = Some(v.to_vec());
            Ok(())
        }
        fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, ErroApp> {
            Ok(self.0.lock().unwrap().clone().map(Zeroizing::new))
        }
        fn limpar(&self) -> Result<(), ErroApp> {
            *self.0.lock().unwrap() = None;
            Ok(())
        }
    }

    fn servico() -> (Servico<DaemonFalso, Arc<Area>>, Arc<Area>) {
        let area = Arc::new(Area::default());
        let s = Servico::novo(DaemonFalso::cheio(), Copiador::novo(Arc::clone(&area)));
        (s, area)
    }

    /// As formas em que um segredo poderia aparecer no JSON que vai à interface.
    fn formas(s: &str) -> Vec<String> {
        let json = serde_json::to_string(s).unwrap();
        let mut v = vec![s.to_owned(), json[1..json.len() - 1].to_owned()];
        // Linha a linha (a chave privada falsa tem várias).
        v.extend(s.lines().filter(|l| l.len() >= 6).map(str::to_owned));
        v
    }

    fn sem_segredo(json: &str, onde: &str) {
        for s in SEGREDOS {
            for f in formas(s) {
                assert!(!json.contains(&f), "segredo em {onde}");
            }
        }
    }

    #[test]
    fn ataque_lista_e_detalhes_de_todas_as_entradas_sem_segredo() {
        // V5.3 do lado Rust: a resposta de cada comando da tela Credenciais, como vai à
        // interface (JSON), não tem nenhum segredo do cofre de teste, em nenhuma forma.
        let (s, _) = servico();
        let lista = s.listar_credenciais().unwrap();
        assert_eq!(
            lista.len(),
            serde_json::from_str::<serde_json::Value>(include_str!(
                "../../../dev/fixtures/entradas.json"
            ))
            .unwrap()["entradas"]
                .as_array()
                .unwrap()
                .len()
        );
        sem_segredo(&serde_json::to_string(&lista).unwrap(), "lista");
        for item in &lista {
            let d = s.detalhe_credencial(&item.nome).unwrap();
            let json = serde_json::to_string(&d).unwrap();
            sem_segredo(&json, &item.nome);
            assert!(!json.contains("valor\":\"falso"), "{}", item.nome);
        }
        // O extra secreto vai só com o nome.
        let g = s.detalhe_credencial("gama-generico").unwrap();
        let licenca = g.extras.iter().find(|x| x.nome == "CHAVE_LICENCA").unwrap();
        assert!(licenca.segredo && licenca.valor.is_none());
        assert_eq!(g.anexos.len(), 1);
    }

    #[test]
    fn lista_e_detalhe_do_acme_pg_dev_como_no_frame() {
        let (s, _) = servico();
        let lista = s.listar_credenciais().unwrap();
        let pg = lista.iter().find(|i| i.nome == "acme-pg-dev").unwrap();
        assert_eq!(pg.resumo, "postgres · app@127.0.0.1:55432");
        assert_eq!((pg.tipo, pg.ambiente, pg.nivel), ("banco", "dev", "livre"));
        let tunel = lista.iter().find(|i| i.nome == "acme-pg-tunel").unwrap();
        assert_eq!(tunel.resumo, "postgres · postgres:5432 · túnel SSH");
        let d = s.detalhe_credencial("acme-pg-dev").unwrap();
        assert_eq!(d.subtitulo, "grupo acme · postgres");
        let rotulos: Vec<_> = d.campos.iter().map(|c| c.rotulo.as_str()).collect();
        assert_eq!(rotulos, ["Host", "Porta", "Base", "Usuário"]);
        assert_eq!(d.segredos.len(), 1);
        assert_eq!(d.segredos[0].rotulo, "Senha");
    }

    #[test]
    fn ataque_copiar_nao_devolve_o_valor() {
        let (s, area) = servico();
        // O tipo da resposta é `()`: não há valor para a interface ver.
        let resposta: Result<(), ErroApp> = s.copiar_campo("acme-pg-dev", "senha");
        assert_eq!(resposta, Ok(()));
        assert_eq!(format!("{resposta:?}"), "Ok(())");
        assert_eq!(
            area.0.lock().unwrap().as_deref(),
            Some(&b"falso-PG:s3nha/01@#"[..])
        );
        // Campo de texto: o valor vem do detalhe do daemon.
        s.copiar_campo("acme-pg-dev", "host").unwrap();
        assert_eq!(area.0.lock().unwrap().as_deref(), Some(&b"127.0.0.1"[..]));
        // Extra secreto, pelo `copiar` com alvo extra.
        s.copiar_campo("gama-generico", "CHAVE_LICENCA").unwrap();
        assert_eq!(
            area.0.lock().unwrap().as_deref(),
            Some(&b"falso-licenca-01"[..])
        );
        s.ao_sair();
        assert_eq!(*area.0.lock().unwrap(), None);
    }

    fn opcoes() -> crate::gerador::Opcoes {
        crate::gerador::Opcoes {
            tamanho: 24,
            minusculas: true,
            maiusculas: true,
            digitos: true,
            simbolos: true,
        }
    }

    /// V5.21 com V5.5: "copiar" do gerador passa pelo mesmo `Copiador` (a área recebe o
    /// valor gerado, a resposta é `()`), e "gerar" não devolve o valor.
    #[test]
    fn ataque_gerador_copia_sem_devolver_o_valor() {
        let area = Arc::new(Area::default());
        let s = Servico::novo(DaemonFalso::cheio(), Copiador::novo(Arc::clone(&area)));
        assert_eq!(s.copiar_senha_gerada(), Err(ErroApp::SemSenhaGerada));
        let g = s.gerar_senha(opcoes()).unwrap();
        let resposta: Result<(), ErroApp> = s.copiar_senha_gerada();
        assert_eq!(resposta, Ok(()));
        let copiado = area.0.lock().unwrap().clone().unwrap();
        assert_eq!(copiado.len(), 24);
        assert!(!format!("{g:?}").contains(core::str::from_utf8(&copiado).unwrap()));
        assert_eq!(
            s.revelar_senha_gerada().unwrap().expor().as_bytes(),
            &copiado[..]
        );
        // Sair da tela zera; "copiar" e "revelar" deixam de ter o que entregar.
        s.descartar_senha_gerada().unwrap();
        assert_eq!(s.copiar_senha_gerada(), Err(ErroApp::SemSenhaGerada));
        assert_eq!(
            s.revelar_senha_gerada().err(),
            Some(ErroApp::SemSenhaGerada)
        );
        // Fechar o app limpa a área (ainda com o valor) e zera a senha.
        s.gerar_senha(opcoes()).unwrap();
        s.copiar_senha_gerada().unwrap();
        s.ao_sair();
        assert_eq!(*area.0.lock().unwrap(), None);
        assert_eq!(s.copiar_senha_gerada(), Err(ErroApp::SemSenhaGerada));
    }

    /// V5.5 no gerador: o texto que o usuário copiou depois não é apagado pela limpeza.
    #[test]
    fn ataque_limpeza_do_gerador_nao_apaga_texto_do_usuario() {
        let area = Arc::new(Area::default());
        let s = Servico::novo(
            DaemonFalso::cheio(),
            Copiador::com_prazo(Arc::clone(&area), std::time::Duration::from_millis(50)),
        );
        s.gerar_senha(opcoes()).unwrap();
        s.copiar_senha_gerada().unwrap();
        *area.0.lock().unwrap() = Some(b"texto do usuario".to_vec());
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(
            area.0.lock().unwrap().as_deref(),
            Some(&b"texto do usuario"[..])
        );
        // Sem cópia nova do usuário, a limpeza apaga o valor gerado.
        s.copiar_senha_gerada().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(*area.0.lock().unwrap(), None);
    }

    /// Fase 39f (S12, baixo da auditoria do M5): a trava que vem de fora (inatividade,
    /// `ward travar`, tela bloqueada), vista pela bandeja, limpa a área de transferência
    /// na hora, se ela ainda tem o valor do Ward; o texto que o usuário copiou depois
    /// fica; a consulta periódica com o cofre destravado não limpa.
    #[test]
    fn ataque_trava_de_fora_limpa_a_area_na_hora() {
        use crate::bandeja::EstadoBandeja::*;
        for depois in [Travado, SemDaemon, SemCofre] {
            let (s, area) = servico();
            s.copiar_campo("acme-pg-dev", "senha").unwrap();
            s.na_passagem_da_bandeja(Destravado, Destravado);
            s.na_passagem_da_bandeja(Travado, Destravado);
            assert!(area.0.lock().unwrap().is_some(), "{depois:?}");
            s.na_passagem_da_bandeja(Destravado, depois);
            assert_eq!(*area.0.lock().unwrap(), None, "{depois:?}");

            s.copiar_campo("acme-pg-dev", "senha").unwrap();
            *area.0.lock().unwrap() = Some(b"texto do usuario".to_vec());
            s.na_passagem_da_bandeja(Destravado, depois);
            assert_eq!(
                area.0.lock().unwrap().as_deref(),
                Some(&b"texto do usuario"[..]),
                "{depois:?}"
            );
        }
    }

    /// Ataque (Fase 39g): o cofre trava por fora (`ward travar`, inatividade) com o
    /// valor copiado, num app **sem bandeja**. A vigia vê a passagem pelo `visto` do app
    /// e limpa a área na hora; antes, sem bandeja, nada limpava até o prazo de 30 s.
    #[test]
    fn ataque_trava_de_fora_sem_bandeja_limpa_a_area_na_hora() {
        use crate::bandeja::EstadoBandeja::*;
        let (s, area) = servico();
        let visto = Mutex::new(SemDaemon);
        assert_eq!(s.ver_passagem(&visto), (SemDaemon, Destravado));
        s.copiar_campo("acme-pg-dev", "senha").unwrap();
        // Consulta sem mudança não limpa.
        assert_eq!(s.ver_passagem(&visto), (Destravado, Destravado));
        assert!(area.0.lock().unwrap().is_some());
        // A trava vem de fora do app: direto no daemon, sem passar pelo `travar` dele.
        cliente::travar(&mut *s.daemon().unwrap()).unwrap();
        assert!(area.0.lock().unwrap().is_some());
        assert_eq!(s.ver_passagem(&visto), (Destravado, Travado));
        assert_eq!(*area.0.lock().unwrap(), None);
        assert_eq!(*visto.lock().unwrap(), Travado);
    }

    #[test]
    fn ataque_erro_nao_carrega_o_pedido() {
        let (s, _) = servico();
        let e = s.copiar_campo("acme-pg-dev\n", "senha").unwrap_err();
        assert_eq!(e, ErroApp::PedidoInvalido);
        assert!(!e.to_string().contains("acme"));
        assert_eq!(
            s.copiar_campo("acme-pg-dev", "inventado").unwrap_err(),
            ErroApp::CampoInexistente
        );
        assert_eq!(s.estado_do_cofre(), Ok(EstadoCofre::Destravado));
    }

    #[test]
    fn ataque_nome_de_entrada_fora_do_formato_nem_chega_ao_daemon() {
        let (s, _) = servico();
        let longo = "a".repeat(ward_core::limites::MAX_NOME + 1);
        for nome in [
            "",
            " acme-pg-dev",
            "acme-pg-dev\n",
            "acme\u{202E}gp",
            "acme\u{0}",
            longo.as_str(),
        ] {
            assert_eq!(
                s.detalhe_credencial(nome).unwrap_err(),
                ErroApp::PedidoInvalido,
                "{nome:?}"
            );
        }
        assert_eq!(
            s.detalhe_credencial("nao-existe").unwrap_err(),
            ErroApp::EntradaInexistente
        );
    }

    fn form(json: serde_json::Value) -> Formulario {
        serde_json::from_value(json).unwrap()
    }

    fn login_novo(nome: &str) -> serde_json::Value {
        serde_json::json!({
            "tipo": "login", "nome": nome, "grupo": "acme", "ambiente": "dev",
            "projeto": "site", "tags": ["wordpress", "painel"],
            "campos": {"url": "https://painel.exemplo.com/login", "usuario": "wp_novo"},
            "segredos": {"senha": {"novo": "falso-nova-senha-77"},
                          "totp_semente": {"novo": "jbsw y3dp ehpk 3pxp"}},
            "extras": [{"nome": "SITE_ID", "segredo": false, "valor": {"novo": "acme-site-01"}}]
        })
    }

    /// Fase 33 (V5.10): cria um `login` (com a semente normalizada) e o edita mantendo a
    /// senha guardada, sem que o valor volte à interface.
    #[test]
    fn salvar_novo_e_editar_mantendo_segredo() {
        let (s, _) = servico();
        let r = s
            .salvar_credencial(None, form(login_novo("acme-wp-novo")))
            .unwrap();
        assert_eq!(r.salvo.as_deref(), Some("acme-wp-novo"), "{r:?}");
        let d = s.detalhe_credencial("acme-wp-novo").unwrap();
        assert_eq!(d.nivel, "com-aprovacao");
        assert_eq!(d.tags, ["wordpress", "painel"]);
        assert_eq!(
            s.revelar_campo("acme-wp-novo", "totp_semente")
                .unwrap()
                .expor(),
            "JBSWY3DPEHPK3PXP"
        );
        assert_eq!(s.codigo_totp("acme-wp-novo").unwrap().codigo.len(), 6);
        // Edição de uma entrada `livre`: mantém a senha, renomeia, o nível fica.
        let edicao = serde_json::json!({
            "tipo": "login", "nome": "acme-totp-dev2", "grupo": "acme", "ambiente": "dev",
            "projeto": "", "tags": [],
            "campos": {"url": "http://localhost:8083/login", "usuario": "wp_admin"},
            "segredos": {"senha": "manter", "totp_semente": "manter"},
            "extras": []
        });
        let r = s
            .salvar_credencial(Some("acme-totp-dev"), form(edicao))
            .unwrap();
        assert_eq!(r.salvo.as_deref(), Some("acme-totp-dev2"), "{r:?}");
        assert_eq!(
            s.detalhe_credencial("acme-totp-dev2").unwrap().nivel,
            "livre"
        );
        assert_eq!(
            s.revelar_campo("acme-totp-dev2", "senha").unwrap().expor(),
            "falso-WP-senha-01"
        );
        assert_eq!(
            s.detalhe_credencial("acme-totp-dev").unwrap_err(),
            ErroApp::EntradaInexistente
        );
    }

    /// V5.10: salvar sem obrigatório volta o erro no campo (do `ward-core`), sem valor.
    #[test]
    fn salvar_sem_obrigatorio_mostra_erro_no_campo() {
        let (s, _) = servico();
        let vazio = serde_json::json!({
            "tipo": "login", "nome": "", "grupo": "", "ambiente": "", "projeto": "",
            "tags": [], "campos": {}, "segredos": {"senha": {"novo": "falso-x-123456"}}, "extras": []
        });
        let r = s.salvar_credencial(None, form(vazio)).unwrap();
        assert_eq!(r.salvo, None);
        let campos: Vec<_> = r.erros.iter().map(|e| e.campo).collect();
        for c in ["nome", "grupo", "ambiente", "url", "usuario"] {
            assert!(campos.contains(&c), "{c}: {r:?}");
        }
        assert!(!serde_json::to_string(&r).unwrap().contains("falso-x"));
        // Nome repetido: erro do daemon no campo nome.
        let r = s
            .salvar_credencial(None, form(login_novo("acme-wp-dev")))
            .unwrap();
        assert!(r.erros.iter().any(|e| e.campo == "nome"), "{r:?}");
    }

    /// D35/V5.13: `otpauth://` com SHA256, 8 dígitos ou 60 s é recusado ao salvar, com a
    /// mensagem fixa no campo da semente, e nada é gravado.
    #[test]
    fn ataque_otpauth_nao_suportado_nao_e_salvo() {
        let (s, _) = servico();
        for uri in [
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&algorithm=SHA256",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&digits=8",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&period=60",
        ] {
            let mut j = login_novo("acme-otp-novo");
            j["segredos"]["totp_semente"] = serde_json::json!({ "novo": uri });
            let r = s.salvar_credencial(None, form(j)).unwrap();
            assert_eq!(r.salvo, None);
            assert_eq!(r.erros[0].campo, "totp_semente");
            assert_eq!(r.erros[0].motivo, "parâmetros de TOTP não suportados");
            assert_eq!(
                s.detalhe_credencial("acme-otp-novo").unwrap_err(),
                ErroApp::EntradaInexistente
            );
        }
        let mut j = login_novo("acme-otp-novo");
        j["segredos"]["totp_semente"] = serde_json::json!({
            "novo": "otpauth://totp/Acme?secret=JBSWY3DPEHPK3PXP&algorithm=SHA1&digits=6&period=30"
        });
        assert!(s.salvar_credencial(None, form(j)).unwrap().salvo.is_some());
    }

    /// V5.14: o gerador do formulário preenche a senha no lado Rust: "usar" devolve só o
    /// tamanho, e o salvo é a senha gerada.
    #[test]
    fn gerador_do_formulario_preenche_a_senha_sem_passar_pela_interface() {
        let (s, _) = servico();
        assert_eq!(s.usar_senha_gerada("senha"), Err(ErroApp::SemSenhaGerada));
        s.gerar_senha(crate::gerador::Opcoes {
            tamanho: 32,
            ..opcoes()
        })
        .unwrap();
        let gerada = s.revelar_senha_gerada().unwrap().expor().to_owned();
        assert_eq!(s.usar_senha_gerada("senha"), Ok(32));
        for campo in ["token", "totp_semente", "", "senha "] {
            assert_eq!(
                s.usar_senha_gerada(campo),
                Err(ErroApp::PedidoInvalido),
                "{campo:?}"
            );
        }
        let mut j = login_novo("acme-gerada");
        j["segredos"]["senha"] = serde_json::json!("gerada");
        let r = s.salvar_credencial(None, form(j)).unwrap();
        assert_eq!(r.salvo.as_deref(), Some("acme-gerada"));
        assert_eq!(
            s.revelar_campo("acme-gerada", "senha").unwrap().expor(),
            gerada
        );
        // Depois de salvar, a senha usada some.
        let mut j = login_novo("acme-gerada-2");
        j["segredos"]["senha"] = serde_json::json!("gerada");
        assert_eq!(
            s.salvar_credencial(None, form(j)),
            Err(ErroApp::SemSenhaGerada)
        );
    }

    /// S11: "manter" só vale na edição e só para segredo que a entrada editada tem; não
    /// dá para puxar o segredo de outra entrada para uma nova. Tipo não muda na edição,
    /// chave de campo de outro tipo é recusada, e entrada com anexos não é editada (os
    /// anexos seriam apagados).
    #[test]
    fn ataque_formulario_forjado_pela_interface() {
        let (s, _) = servico();
        let mut j = login_novo("acme-roubo");
        j["segredos"]["senha"] = serde_json::json!("manter");
        assert_eq!(
            s.salvar_credencial(None, form(j)),
            Err(ErroApp::PedidoInvalido)
        );
        let mut j = login_novo("acme-roubo");
        j["extras"] = serde_json::json!([
            {"nome": "X", "segredo": false, "valor": {"manter": "CHAVE_LICENCA"}}
        ]);
        assert_eq!(
            s.salvar_credencial(None, form(j)),
            Err(ErroApp::PedidoInvalido)
        );
        // Na edição de outra entrada, o extra que ela não tem também não vale.
        let mut e = login_novo("acme-wp-dev");
        e["extras"] = serde_json::json!([
            {"nome": "X", "segredo": true, "valor": {"manter": "CHAVE_LICENCA"}}
        ]);
        assert_eq!(
            s.salvar_credencial(Some("acme-wp-dev"), form(e)),
            Err(ErroApp::PedidoInvalido)
        );
        // Tipo trocado na edição.
        let mut t = login_novo("beta-github-api");
        t["campos"] = serde_json::json!({});
        assert_eq!(
            s.salvar_credencial(Some("beta-github-api"), form(t)),
            Err(ErroApp::PedidoInvalido)
        );
        // Campo de outro tipo.
        let mut c = login_novo("acme-x");
        c["campos"]["host"] = serde_json::json!("127.0.0.1");
        assert_eq!(
            s.salvar_credencial(None, form(c)),
            Err(ErroApp::PedidoInvalido)
        );
        // Campos de login num tipo da Fase 34, e tipo inventado.
        for tipo in ["banco", "servidor", "ftp-sftp", "root", ""] {
            let mut c = login_novo("acme-x");
            c["tipo"] = serde_json::json!(tipo);
            assert_eq!(
                s.salvar_credencial(None, form(c)),
                Err(ErroApp::PedidoInvalido),
                "{tipo}"
            );
        }
        // Anexos mantidos forjados (Fase 39b): nome que a entrada não tem, repetido ou
        // numa entrada nova; nada é gravado.
        for (anterior, mantidos) in [
            (Some("gama-generico"), serde_json::json!(["outro.env"])),
            (
                Some("gama-generico"),
                serde_json::json!(["app.env", "app.env"]),
            ),
            (Some("gama-generico"), serde_json::json!(["../app.env"])),
            (None, serde_json::json!(["app.env"])),
        ] {
            let mut g = serde_json::json!({
                "tipo": "generico", "nome": "gama-generico-2", "grupo": "gama",
                "ambiente": "stage", "projeto": "", "tags": [], "campos": {}, "segredos": {},
                "extras": [{"nome": "REGIAO", "segredo": false, "valor": {"novo": "r-02"}}]
            });
            g["anexos_mantidos"] = mantidos;
            assert_eq!(
                s.salvar_credencial(anterior, form(g)),
                Err(ErroApp::PedidoInvalido),
                "{anterior:?}"
            );
        }
        assert_eq!(
            s.detalhe_credencial("gama-generico").unwrap().anexos.len(),
            1
        );
        assert_eq!(
            s.detalhe_credencial("gama-generico-2").unwrap_err(),
            ErroApp::EntradaInexistente
        );
        // Nome anterior fora do formato nem chega ao daemon.
        assert_eq!(
            s.salvar_credencial(Some("acme\n"), form(login_novo("x"))),
            Err(ErroApp::PedidoInvalido)
        );
    }

    /// V5.3/V5.11 (D65 b): só o `revelar_campo` leva valor; nenhuma outra resposta do
    /// formulário (salvar, TOTP) carrega segredo do cofre. Campo de texto não é
    /// "revelado".
    #[test]
    fn ataque_revelar_so_de_segredo_e_respostas_sem_segredo() {
        let (s, area) = servico();
        assert_eq!(
            s.revelar_campo("acme-pg-dev", "senha").unwrap().expor(),
            "falso-PG:s3nha/01@#"
        );
        assert_eq!(
            s.revelar_campo("gama-generico", "CHAVE_LICENCA")
                .unwrap()
                .expor(),
            "falso-licenca-01"
        );
        assert_eq!(
            s.revelar_campo("gama-generico", "REGIAO").unwrap_err(),
            ErroApp::CampoInexistente
        );
        assert_eq!(
            s.revelar_campo("acme-pg-dev", "host").unwrap_err(),
            ErroApp::CampoInexistente
        );
        assert_eq!(
            s.revelar_campo("acme-pg-dev\n", "senha").unwrap_err(),
            ErroApp::PedidoInvalido
        );
        assert!(
            !format!("{:?}", s.revelar_campo("acme-pg-dev", "senha").unwrap()).contains("falso")
        );
        let totp = serde_json::to_string(&s.codigo_totp("acme-wp-dev").unwrap()).unwrap();
        sem_segredo(&totp, "totp");
        assert_eq!(
            s.codigo_totp("acme-pg-dev").unwrap_err(),
            ErroApp::CampoInexistente
        );
        s.copiar_codigo_totp("acme-wp-dev").unwrap();
        assert_eq!(area.0.lock().unwrap().as_ref().map(Vec::len), Some(6));
        let mut j = login_novo("acme-edit");
        j["segredos"]["senha"] = serde_json::json!({"novo": "falso-WP-senha-01"});
        let r = serde_json::to_string(&s.salvar_credencial(None, form(j)).unwrap()).unwrap();
        sem_segredo(&r, "salvar");
        for item in s.listar_credenciais().unwrap() {
            let d = serde_json::to_string(&s.detalhe_credencial(&item.nome).unwrap()).unwrap();
            sem_segredo(&d, &item.nome);
        }
    }

    /// D65 c: a lista da tela sai de uma troca só (sem um `detalhe` por entrada), com a
    /// 2ª linha e o usuário da busca tirados do resumo.
    #[test]
    fn d65_lista_numa_troca_so_com_resumo_e_usuario() {
        struct Contador(DaemonFalso, Arc<std::sync::atomic::AtomicUsize>);
        impl Daemon for Contador {
            fn trocar(&mut self, p: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
                self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                self.0.trocar(p)
            }
        }
        let n = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let s = Servico::novo(
            Contador(DaemonFalso::cheio(), Arc::clone(&n)),
            Copiador::novo(Arc::new(Area::default())),
        );
        let lista = s.listar_credenciais().unwrap();
        assert_eq!(n.load(std::sync::atomic::Ordering::SeqCst), 1);
        let pg = lista.iter().find(|i| i.nome == "acme-pg-dev").unwrap();
        assert_eq!(pg.usuario.as_deref(), Some("app"));
        let totp = lista.iter().find(|i| i.nome == "acme-totp-dev").unwrap();
        assert_eq!(totp.usuario.as_deref(), Some("wp_admin"));
        assert!(totp.resumo.ends_with("TOTP"), "{}", totp.resumo);
        let g = lista.iter().find(|i| i.nome == "gama-generico").unwrap();
        assert_eq!(g.resumo, "2 campos extras · 1 anexo");
        sem_segredo(&serde_json::to_string(&lista).unwrap(), "lista");
    }

    #[test]
    fn cofre_vazio() {
        let area = Arc::new(Area::default());
        let s = Servico::novo(DaemonFalso::vazio(), Copiador::novo(area));
        assert!(s.listar_credenciais().unwrap().is_empty());
    }

    const PIN_CERTO: &str = "135790";
    const PIN_ERRADO: &str = "000000";

    fn pin(p: &str) -> Pin {
        Pin::novo(p.to_owned()).unwrap()
    }

    fn nivel_de(s: &Servico<DaemonFalso, Arc<Area>>, entrada: &str) -> &'static str {
        s.detalhe_credencial(entrada).unwrap().nivel
    }

    #[test]
    fn mudar_nivel_com_pin_e_os_tres_niveis_tambem_em_prod() {
        // V5.15: com o PIN certo o nível muda, na lista e no detalhe; em prod os três
        // níveis valem igual (o ambiente não muda as regras).
        let (s, _) = servico();
        assert_eq!(nivel_de(&s, "acme-pg-prod"), "com-aprovacao");
        for nivel in ["livre", "bloqueado", "com-aprovacao", "livre"] {
            let r = s
                .mudar_nivel("acme-pg-prod", nivel, pin(PIN_CERTO))
                .unwrap();
            assert_eq!((r.entrada.as_str(), r.nivel), ("acme-pg-prod", nivel));
            assert_eq!(nivel_de(&s, "acme-pg-prod"), nivel);
            let item = s
                .listar_credenciais()
                .unwrap()
                .into_iter()
                .find(|i| i.nome == "acme-pg-prod")
                .unwrap();
            assert_eq!(item.nivel, nivel);
        }
        assert_eq!(
            s.mudar_nivel("nao-existe", "livre", pin(PIN_CERTO))
                .unwrap_err(),
            ErroApp::EntradaInexistente
        );
    }

    #[test]
    fn ataque_pin_errado_nao_muda_e_o_quinto_erro_trava() {
        // S9 / V5.15: PIN errado responde "PIN incorreto" e o nível não muda; 4 erros e
        // 1 acerto zeram a contagem; o 5º erro seguido trava o cofre, e travado nem o
        // PIN certo muda o nível.
        let (s, _) = servico();
        let errar = || {
            s.mudar_nivel("acme-pg-dev", "bloqueado", pin(PIN_ERRADO))
                .unwrap_err()
        };
        for _ in 0..4 {
            assert_eq!(errar(), ErroApp::PinIncorreto);
            assert_eq!(nivel_de(&s, "acme-pg-dev"), "livre");
            assert_eq!(s.estado_do_cofre(), Ok(EstadoCofre::Destravado));
        }
        s.mudar_nivel("acme-pg-dev", "livre", pin(PIN_CERTO))
            .unwrap();
        for _ in 0..4 {
            assert_eq!(errar(), ErroApp::PinIncorreto);
        }
        assert_eq!(s.estado_do_cofre(), Ok(EstadoCofre::Destravado));
        assert_eq!(nivel_de(&s, "acme-pg-dev"), "livre");
        assert_eq!(errar(), ErroApp::PinIncorreto);
        assert_eq!(s.estado_do_cofre(), Ok(EstadoCofre::Travado));
        assert_eq!(
            s.mudar_nivel("acme-pg-dev", "bloqueado", pin(PIN_CERTO))
                .unwrap_err(),
            ErroApp::Travado
        );
        assert_eq!(
            s.detalhe_credencial("acme-pg-dev").unwrap_err(),
            ErroApp::Travado
        );
    }

    #[test]
    fn ataque_nome_ou_nivel_forjado_nem_chega_ao_daemon() {
        // Nome fora do formato ou nível inventado são recusados antes do daemon: não
        // mudam nada e não contam como erro de PIN (nem com o PIN errado).
        let (s, _) = servico();
        for _ in 0..6 {
            for (entrada, nivel) in [
                ("acme-pg-dev", "admin"),
                ("acme-pg-dev", "Livre"),
                ("acme-pg-dev", ""),
                ("acme\npg", "bloqueado"),
                ("", "bloqueado"),
                (" acme-pg-dev", "bloqueado"),
            ] {
                assert_eq!(
                    s.mudar_nivel(entrada, nivel, pin(PIN_ERRADO)).unwrap_err(),
                    ErroApp::PedidoInvalido,
                    "{entrada:?} {nivel:?}"
                );
            }
        }
        assert_eq!(s.estado_do_cofre(), Ok(EstadoCofre::Destravado));
        assert_eq!(nivel_de(&s, "acme-pg-dev"), "livre");
    }

    #[test]
    fn ataque_resposta_e_erro_do_mudar_nivel_sem_pin() {
        // A resposta (JSON que vai à interface) e a mensagem do erro não trazem o PIN
        // nem segredo do cofre.
        let (s, _) = servico();
        let r = s
            .mudar_nivel("acme-wp-dev", "livre", pin(PIN_CERTO))
            .unwrap();
        let ok = serde_json::to_string(&r).unwrap();
        assert!(!ok.contains(PIN_CERTO), "{ok}");
        sem_segredo(&ok, "mudar_nivel");
        let e = s
            .mudar_nivel("acme-wp-dev", "livre", pin(PIN_ERRADO))
            .unwrap_err();
        let texto = format!("{e} {e:?} {}", e.codigo());
        assert!(!texto.chars().any(|c| c.is_ascii_digit()), "{texto}");
    }

    // ---- Fase 34: banco, servidor, ftp/sftp, anexos e "Conectar" ----

    use crate::anexos::Escolha;
    use crate::terminal::Token;
    use std::path::PathBuf;

    /// Seletor de teste: devolve o caminho que o teste pôs.
    #[derive(Clone, Default)]
    struct SeletorFixo(Arc<Mutex<Option<PathBuf>>>);

    impl crate::anexos::Seletor for SeletorFixo {
        fn escolher(&self) -> Result<Option<PathBuf>, ErroApp> {
            Ok(self.0.lock().unwrap().clone())
        }
    }

    /// Terminal de teste: guarda entrada e token pedidos.
    #[derive(Clone, Default)]
    struct TerminalFixo(Arc<Mutex<Vec<(String, String)>>>);

    impl crate::terminal::Terminal for TerminalFixo {
        fn abrir(&self, entrada: &str, token: &Token) -> Result<(), ErroApp> {
            // `Token` não expõe o valor fora do módulo; o teste só vê o tamanho e o Debug.
            self.0
                .lock()
                .unwrap()
                .push((entrada.to_owned(), format!("{token:?}:{}", token.len())));
            Ok(())
        }
    }

    fn servico_34() -> (Servico<DaemonFalso, Arc<Area>>, SeletorFixo, TerminalFixo) {
        let (s, _) = servico();
        let sel = SeletorFixo::default();
        let ter = TerminalFixo::default();
        (
            s.com_seletor(sel.clone()).com_terminal(ter.clone()),
            sel,
            ter,
        )
    }

    fn pasta_34(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ward-servico-34-{}-{nome}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn chave_de_teste() -> String {
        // Montada em pedaços: o texto inteiro de uma chave não fica no repositório.
        [
            "-----BEGIN ",
            "OPENSSH",
            " PRIVATE",
            " KEY-----\n",
            "falso-chave-fase34\n",
            "-----END ",
            "OPENSSH",
            " PRIVATE",
            " KEY-----\n",
        ]
        .concat()
    }

    fn servidor_novo(nome: &str) -> serde_json::Value {
        serde_json::json!({
            "tipo": "servidor", "nome": nome, "grupo": "clientex", "ambiente": "stage",
            "projeto": "", "tags": [],
            "campos": {"host": "127.0.0.1", "porta": "2222", "usuario": "deploy", "salto": ""},
            "segredos": {"senha": "vazio", "chave_privada": {"novo": chave_de_teste()},
                "passphrase": {"novo": "falso-frase-34"}},
            "extras": [], "anexos": []
        })
    }

    /// V5.10: os três tipos da fase salvam e editam (mantendo o segredo, o nível fica).
    #[test]
    fn banco_servidor_e_ftp_salvam_e_editam() {
        let (s, _, _) = servico_34();
        let banco = serde_json::json!({
            "tipo": "banco", "nome": "clientex-pg-stage", "grupo": "clientex", "ambiente": "stage",
            "projeto": "", "tags": [],
            "campos": {"motor": "postgres", "host": "127.0.0.1", "porta": "", "base": "app_teste",
                "usuario": "app", "tunel": "acme-ssh-senha-stage"},
            "segredos": {"senha": {"novo": "falso-banco-34"}},
            "extras": [], "anexos": []
        });
        let r = s.salvar_credencial(None, form(banco)).unwrap();
        assert_eq!(r.salvo.as_deref(), Some("clientex-pg-stage"), "{r:?}");
        let d = s.detalhe_credencial("clientex-pg-stage").unwrap();
        assert!(
            d.campos
                .iter()
                .any(|c| c.id == "porta" && c.valor == "5432")
        );
        assert!(
            d.campos
                .iter()
                .any(|c| c.id == "tunel" && c.valor == "acme-ssh-senha-stage")
        );

        let r = s
            .salvar_credencial(None, form(servidor_novo("clientex-ssh-stage")))
            .unwrap();
        assert_eq!(r.salvo.as_deref(), Some("clientex-ssh-stage"), "{r:?}");
        assert_eq!(
            s.detalhe_credencial("clientex-ssh-stage")
                .unwrap()
                .segredos
                .len(),
            2
        );

        let ftp = serde_json::json!({
            "tipo": "ftp-sftp", "nome": "clientex-ftp", "grupo": "clientex", "ambiente": "dev",
            "projeto": "", "tags": [],
            "campos": {"protocolo": "ftp", "host": "127.0.0.1", "porta": "", "usuario": "sftpuser"},
            "segredos": {"senha": {"novo": "falso-ftp-34"}, "chave_privada": "vazio", "passphrase": "vazio"},
            "extras": [], "anexos": []
        });
        let r = s.salvar_credencial(None, form(ftp)).unwrap();
        assert_eq!(r.salvo.as_deref(), Some("clientex-ftp"), "{r:?}");

        // Edição do servidor com chave do cofre de teste, mantendo chave e passphrase.
        let ed = serde_json::json!({
            "tipo": "servidor", "nome": "acme-ssh-chave-prod", "grupo": "acme", "ambiente": "prod",
            "projeto": "", "tags": [],
            "campos": {"host": "127.0.0.1", "porta": "2223", "usuario": "deploy", "salto": ""},
            "segredos": {"senha": "vazio", "chave_privada": "manter", "passphrase": "manter"},
            "extras": [], "anexos": []
        });
        let r = s
            .salvar_credencial(Some("acme-ssh-chave-prod"), form(ed))
            .unwrap();
        assert_eq!(r.salvo.as_deref(), Some("acme-ssh-chave-prod"), "{r:?}");
        let chave = s
            .revelar_campo("acme-ssh-chave-prod", "chave_privada")
            .unwrap();
        assert!(chave.expor().contains("falso-chave-ed25519-teste"));
    }

    /// V5.10 e D33: os erros dos tipos novos voltam no campo certo, sem valor.
    #[test]
    fn erros_dos_tipos_novos_no_campo() {
        let (s, _, _) = servico_34();
        let banco = serde_json::json!({
            "tipo": "banco", "nome": "clientex-pg", "grupo": "clientex", "ambiente": "dev",
            "projeto": "", "tags": [],
            "campos": {"motor": "postgres", "host": "127.0.0.1", "porta": "99999", "base": "app.teste?x",
                "usuario": "app", "tunel": "acme-pg-dev"},
            "segredos": {"senha": {"novo": "falso-banco-erro"}},
            "extras": [], "anexos": []
        });
        let r = s.salvar_credencial(None, form(banco)).unwrap();
        let campos: Vec<_> = r.erros.iter().map(|e| e.campo).collect();
        assert!(
            campos.contains(&"base") && campos.contains(&"porta"),
            "{r:?}"
        );
        assert!(!serde_json::to_string(&r).unwrap().contains("falso-banco"));
        // Túnel que não é servidor: erro do daemon no campo do túnel.
        let mut b = serde_json::json!({
            "tipo": "banco", "nome": "clientex-pg", "grupo": "clientex", "ambiente": "dev",
            "projeto": "", "tags": [],
            "campos": {"motor": "postgres", "host": "127.0.0.1", "porta": "", "base": "app",
                "usuario": "app", "tunel": "acme-pg-dev"},
            "segredos": {"senha": {"novo": "falso-banco-erro"}},
            "extras": [], "anexos": []
        });
        let r = s.salvar_credencial(None, form(b.clone())).unwrap();
        assert_eq!(r.erros.first().map(|e| e.campo), Some("tunel"), "{r:?}");
        b["campos"]["motor"] = serde_json::json!("oracle");
        b["campos"]["tunel"] = serde_json::json!("");
        let r = s.salvar_credencial(None, form(b)).unwrap();
        assert!(r.erros.iter().any(|e| e.campo == "motor"), "{r:?}");
        // Sem senha nem chave no servidor: erro em "autenticacao".
        let mut v = servidor_novo("clientex-ssh");
        v["segredos"] =
            serde_json::json!({"senha": "vazio", "chave_privada": "vazio", "passphrase": "vazio"});
        let r = s.salvar_credencial(None, form(v)).unwrap();
        assert!(r.erros.iter().any(|e| e.campo == "autenticacao"), "{r:?}");
    }

    /// S1: a interface forjando os dois métodos, ou chave no ftp, é recusada pela
    /// validação; "manter" da chave numa entrada que só tem senha é pedido inválido.
    #[test]
    fn ataque_autenticacao_forjada_pela_interface() {
        let (s, _, _) = servico_34();
        let mut v = servidor_novo("clientex-ssh");
        v["segredos"]["senha"] = serde_json::json!({"novo": "falso-as-duas-34"});
        let r = s.salvar_credencial(None, form(v)).unwrap();
        assert!(r.erros.iter().any(|e| e.campo == "autenticacao"), "{r:?}");
        let mut f = servidor_novo("clientex-ftp");
        f["tipo"] = serde_json::json!("ftp-sftp");
        f["campos"] = serde_json::json!({"protocolo": "ftp", "host": "127.0.0.1", "porta": "", "usuario": "u"});
        let r = s.salvar_credencial(None, form(f)).unwrap();
        assert!(r.erros.iter().any(|e| e.campo == "autenticacao"), "{r:?}");
        let ed = serde_json::json!({
            "tipo": "servidor", "nome": "acme-ssh-senha-stage", "grupo": "acme", "ambiente": "stage",
            "projeto": "", "tags": [],
            "campos": {"host": "127.0.0.1", "porta": "2222", "usuario": "deploy", "salto": ""},
            "segredos": {"senha": "vazio", "chave_privada": "manter", "passphrase": "vazio"},
            "extras": [], "anexos": []
        });
        assert_eq!(
            s.salvar_credencial(Some("acme-ssh-senha-stage"), form(ed)),
            Err(ErroApp::PedidoInvalido)
        );
    }

    /// V5.12 (S11): o anexo é escolhido e lido no Rust; a resposta traz só id, nome e
    /// tamanho; ele vai no rascunho pelo id, e o detalhe mostra nome e tamanho.
    #[test]
    fn ataque_anexo_nao_volta_a_interface_e_vai_no_rascunho() {
        let (s, sel, _) = servico_34();
        let d = pasta_34("anexo");
        let conteudo = "falso-conteudo-do-anexo-34\nLINHA=falso-valor-34\n";
        std::fs::write(d.join("acme-vpn.ovpn"), conteudo).unwrap();
        *sel.0.lock().unwrap() = Some(d.join("acme-vpn.ovpn"));
        let e = s.escolher_anexo(None).unwrap();
        let json = serde_json::to_string(&e).unwrap();
        assert!(
            !json.contains("falso-conteudo") && !json.contains("falso-valor"),
            "{json}"
        );
        let Escolha::Anexado { id, tamanho, .. } = e else {
            panic!("{e:?}")
        };
        assert_eq!(tamanho, conteudo.len() as u64);
        let mut v = servidor_novo("clientex-ssh-anexo");
        v["anexos"] = serde_json::json!([id]);
        let r = s.salvar_credencial(None, form(v)).unwrap();
        assert_eq!(r.salvo.as_deref(), Some("clientex-ssh-anexo"), "{r:?}");
        let det = s.detalhe_credencial("clientex-ssh-anexo").unwrap();
        assert_eq!(det.anexos.len(), 1);
        assert_eq!(det.anexos[0].nome, "acme-vpn.ovpn");
        assert_eq!(det.anexos[0].tamanho, conteudo.len() as u64);
        sem_segredo(&serde_json::to_string(&det).unwrap(), "detalhe com anexo");
        assert!(
            !serde_json::to_string(&det)
                .unwrap()
                .contains("falso-conteudo")
        );
        // Salvo, os anexos escolhidos são descartados: o id não vale de novo.
        let mut v = servidor_novo("clientex-ssh-anexo2");
        v["anexos"] = serde_json::json!([id]);
        assert_eq!(
            s.salvar_credencial(None, form(v)),
            Err(ErroApp::PedidoInvalido)
        );
        let _ = std::fs::remove_dir_all(d);
    }

    fn chave_53a(miolo: &str) -> String {
        let rotulo = ["OPENSSH", " PRIVATE", " KEY"].concat();
        format!("-----BEGIN {rotulo}-----\n{miolo}\n-----END {rotulo}-----\n")
    }

    fn servidor_carregado(nome: &str) -> serde_json::Value {
        let mut v = servidor_novo(nome);
        v["segredos"]["chave_privada"] = serde_json::json!("carregada");
        v
    }

    /// Fase 53a (S2, S11): "Carregar de arquivo" lê a chave no Rust; a resposta traz só o
    /// tamanho; salvar com `"carregada"` grava exatamente o arquivo no cofre (o revelar
    /// do daemon devolve o mesmo); salvar descarta a chave carregada.
    #[test]
    fn carregar_chave_vai_ao_daemon_sem_passar_pela_interface() {
        let (s, sel, _) = servico_34();
        let d = pasta_34("chave53a");
        let k = chave_53a("ZmFsc28tY2hhdmUtc2VydmljbzUzYQ==");
        std::fs::write(d.join("deploy_ed25519"), &k).unwrap();
        *sel.0.lock().unwrap() = Some(d.join("deploy_ed25519"));
        let r = s.carregar_chave("chave_privada").unwrap();
        assert_eq!(
            r,
            Carregamento::Carregada {
                tamanho: k.len() as u64
            }
        );
        let json = serde_json::to_string(&r).unwrap();
        assert!(
            !json.contains("ZmFsc28") && !json.contains("deploy"),
            "{json}"
        );
        let r = s
            .salvar_credencial(None, form(servidor_carregado("clientex-ssh-53a")))
            .unwrap();
        assert_eq!(r.salvo.as_deref(), Some("clientex-ssh-53a"), "{r:?}");
        let v = s
            .revelar_campo("clientex-ssh-53a", "chave_privada")
            .unwrap();
        assert_eq!(v.expor(), k.as_str());
        // Salvo, a chave foi descartada: "carregada" de novo é pedido inválido.
        assert_eq!(
            s.salvar_credencial(None, form(servidor_carregado("clientex-ssh-53b"))),
            Err(ErroApp::PedidoInvalido)
        );
        let _ = std::fs::remove_dir_all(d);
    }

    /// Fase 53d (2ª auditoria do M10): a trava que vem de fora (inatividade, `ward
    /// travar`, tela bloqueada) chega pela bandeja e descarta a chave carregada; a
    /// consulta periódica com o cofre destravado não descarta.
    #[test]
    fn ataque_trava_de_fora_descarta_a_chave_carregada() {
        use crate::bandeja::EstadoBandeja::*;
        let (s, sel, _) = servico_34();
        let d = pasta_34("chave53d");
        std::fs::write(d.join("chave"), chave_53a("ZmFsc28tNTNk")).unwrap();
        *sel.0.lock().unwrap() = Some(d.join("chave"));
        for depois in [Travado, SemDaemon, SemCofre] {
            assert!(matches!(
                s.carregar_chave("chave_privada").unwrap(),
                Carregamento::Carregada { .. }
            ));
            s.na_passagem_da_bandeja(Destravado, depois);
            assert_eq!(
                s.salvar_credencial(None, form(servidor_carregado("clientex-ssh-53d"))),
                Err(ErroApp::PedidoInvalido),
                "{depois:?}"
            );
        }
        assert!(matches!(
            s.carregar_chave("chave_privada").unwrap(),
            Carregamento::Carregada { .. }
        ));
        s.na_passagem_da_bandeja(Destravado, Destravado);
        s.na_passagem_da_bandeja(Travado, Destravado);
        let r = s
            .salvar_credencial(None, form(servidor_carregado("clientex-ssh-53d")))
            .unwrap();
        assert_eq!(r.salvo.as_deref(), Some("clientex-ssh-53d"), "{r:?}");
        let _ = std::fs::remove_dir_all(d);
    }

    /// Fase 53a: campo fora da lista nem abre o seletor; cancelar, aviso e
    /// `descartar_formulario` não deixam chave para o salvar; o aviso não traz o
    /// conteúdo nem o caminho; um aviso não apaga a chave carregada antes.
    #[test]
    fn ataque_carregar_chave_campo_forjado_aviso_e_descartar() {
        let (s, sel, _) = servico_34();
        let d = pasta_34("chave53a-ataque");
        let texto = d.join("segredo-qualquer.txt");
        std::fs::write(&texto, "falso-texto-53a=valor\n").unwrap();
        *sel.0.lock().unwrap() = Some(texto.clone());
        for campo in ["senha", "passphrase", "totp_semente", "", "chave_privada "] {
            assert_eq!(
                s.carregar_chave(campo),
                Err(ErroApp::PedidoInvalido),
                "{campo:?}"
            );
        }
        let r = s.carregar_chave("chave_privada").unwrap();
        assert_eq!(
            r,
            Carregamento::Aviso {
                motivo: chave::AVISO_NAO_E_CHAVE.into()
            }
        );
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("falso-texto") && !json.contains("segredo-qualquer"));
        assert_eq!(
            s.salvar_credencial(None, form(servidor_carregado("clientex-ssh-53c"))),
            Err(ErroApp::PedidoInvalido)
        );
        // Link para a chave: recusado (o seletor devolve o caminho do link).
        let k = chave_53a("ZmFsc28tbGluaw==");
        std::fs::write(d.join("chave"), &k).unwrap();
        std::os::unix::fs::symlink(d.join("chave"), d.join("link")).unwrap();
        *sel.0.lock().unwrap() = Some(d.join("link"));
        assert_eq!(
            s.carregar_chave("chave_privada").unwrap(),
            Carregamento::Aviso {
                motivo: chave::AVISO_LINK.into()
            }
        );
        // Carregada e depois um aviso: a carregada continua; descartar a apaga.
        *sel.0.lock().unwrap() = Some(d.join("chave"));
        assert!(matches!(
            s.carregar_chave("chave_privada").unwrap(),
            Carregamento::Carregada { .. }
        ));
        *sel.0.lock().unwrap() = Some(texto);
        assert!(matches!(
            s.carregar_chave("chave_privada").unwrap(),
            Carregamento::Aviso { .. }
        ));
        s.descartar_formulario();
        assert_eq!(
            s.salvar_credencial(None, form(servidor_carregado("clientex-ssh-53c"))),
            Err(ErroApp::PedidoInvalido)
        );
        // Travar também descarta.
        *sel.0.lock().unwrap() = Some(d.join("chave"));
        assert!(matches!(
            s.carregar_chave("chave_privada").unwrap(),
            Carregamento::Carregada { .. }
        ));
        s.ao_sair();
        assert_eq!(
            s.salvar_credencial(None, form(servidor_carregado("clientex-ssh-53c"))),
            Err(ErroApp::PedidoInvalido)
        );
        // Cancelado: nada.
        *sel.0.lock().unwrap() = None;
        assert_eq!(
            s.carregar_chave("chave_privada").unwrap(),
            Carregamento::Cancelado
        );
        // "carregada" num campo que não é de chave: pedido inválido.
        let mut v = servidor_novo("clientex-ssh-53d");
        v["segredos"]["passphrase"] = serde_json::json!("carregada");
        assert_eq!(
            s.salvar_credencial(None, form(v)),
            Err(ErroApp::PedidoInvalido)
        );
        let _ = std::fs::remove_dir_all(d);
    }

    /// V5.12 (D29): 300 KiB e soma acima de 256 KiB avisam e não são guardados; vazio é
    /// recusado; id forjado ou repetido é pedido inválido; cancelar não guarda nada; o
    /// nome reservado (`CHAVE` no servidor) dá o erro no campo dos anexos.
    #[test]
    fn ataque_anexo_grande_vazio_e_id_forjado() {
        let (s, sel, _) = servico_34();
        let d = pasta_34("limites");
        let escolher = |nome: &str, tamanho: usize| {
            std::fs::write(d.join(nome), vec![b'a'; tamanho]).unwrap();
            *sel.0.lock().unwrap() = Some(d.join(nome));
            s.escolher_anexo(None).unwrap()
        };
        assert!(matches!(
            escolher("grande.bin", 300 * 1024),
            Escolha::Aviso { .. }
        ));
        assert!(matches!(escolher("vazio.txt", 0), Escolha::Aviso { .. }));
        let Escolha::Anexado { id, .. } = escolher("a.pem", 200 * 1024) else {
            panic!()
        };
        assert!(matches!(
            escolher("b.pem", 100 * 1024),
            Escolha::Aviso { .. }
        ));
        *sel.0.lock().unwrap() = None;
        assert_eq!(s.escolher_anexo(None).unwrap(), Escolha::Cancelado);
        for ids in [serde_json::json!([id, id]), serde_json::json!([id + 99])] {
            let mut v = servidor_novo("clientex-ssh-x");
            v["anexos"] = ids;
            assert_eq!(
                s.salvar_credencial(None, form(v)),
                Err(ErroApp::PedidoInvalido)
            );
        }
        s.remover_anexo(id).unwrap();
        assert_eq!(s.remover_anexo(id), Err(ErroApp::PedidoInvalido));
        let Escolha::Anexado { id, .. } = escolher("chave", 10) else {
            panic!()
        };
        let mut v = servidor_novo("clientex-ssh-x");
        v["anexos"] = serde_json::json!([id]);
        let r = s.salvar_credencial(None, form(v)).unwrap();
        assert!(r.erros.iter().any(|e| e.campo == "anexos"), "{r:?}");
        // Cancelar o formulário descarta os escolhidos.
        s.descartar_formulario();
        assert_eq!(s.remover_anexo(id), Err(ErroApp::PedidoInvalido));
        let _ = std::fs::remove_dir_all(d);
    }

    /// Fase 39b (V5.12, S11): editar `gama-generico` mantendo o `app.env`, tirando-o e
    /// pondo outro. O quadro que o app manda ao daemon leva só o nome do anexo mantido,
    /// nunca o conteúdo; nenhuma resposta à interface o traz.
    #[test]
    fn ataque_edicao_mantem_remove_e_adiciona_anexo_sem_o_conteudo_passar_pelo_app() {
        struct Espiao(DaemonFalso, Arc<Mutex<Vec<Vec<u8>>>>);
        impl Daemon for Espiao {
            fn trocar(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
                self.1.lock().unwrap().push(pedido.to_vec());
                let r = self.0.trocar(pedido)?;
                self.1.lock().unwrap().push(r.to_vec());
                Ok(r)
            }
        }
        let quadros = Arc::new(Mutex::new(Vec::new()));
        let sel = SeletorFixo::default();
        let s = Servico::novo(
            Espiao(DaemonFalso::cheio(), Arc::clone(&quadros)),
            Copiador::novo(Arc::new(Area::default())),
        )
        .com_seletor(sel.clone());
        let conteudo = "FALSO_ENV=falso-anexo-01";
        let edicao = |mantidos: serde_json::Value| {
            let mut g = serde_json::json!({
                "tipo": "generico", "nome": "gama-generico", "grupo": "gama",
                "ambiente": "stage", "projeto": "", "tags": [], "campos": {}, "segredos": {},
                "extras": [
                    {"nome": "REGIAO", "segredo": false, "valor": {"novo": "r-02"}},
                    {"nome": "CHAVE_LICENCA", "segredo": true,
                     "valor": {"manter": "CHAVE_LICENCA"}}
                ]
            });
            g["anexos_mantidos"] = mantidos;
            g
        };
        // Manter: o anexo continua, com o mesmo tamanho.
        let antes = s.detalhe_credencial("gama-generico").unwrap().anexos;
        assert_eq!(antes.len(), 1);
        let r = s
            .salvar_credencial(
                Some("gama-generico"),
                form(edicao(serde_json::json!(["app.env"]))),
            )
            .unwrap();
        assert_eq!(r.salvo.as_deref(), Some("gama-generico"), "{r:?}");
        let det = s.detalhe_credencial("gama-generico").unwrap();
        assert_eq!(det.anexos.len(), 1);
        assert_eq!(det.anexos[0].nome, "app.env");
        assert_eq!(det.anexos[0].tamanho, antes[0].tamanho);
        assert_eq!(
            det.extras
                .iter()
                .find(|x| x.nome == "REGIAO")
                .unwrap()
                .valor
                .as_deref(),
            Some("r-02")
        );
        // Adicionar outro, contando o mantido nos limites.
        let d = pasta_34("edicao");
        std::fs::write(d.join(".env"), "OUTRO=falso-outro-39b\n").unwrap();
        *sel.0.lock().unwrap() = Some(d.join(".env"));
        let ed = EdicaoDosAnexos {
            entrada: "gama-generico".into(),
            mantidos: vec!["app.env".into()],
        };
        let Escolha::Anexado { id, nome, .. } = s.escolher_anexo(Some(&ed)).unwrap() else {
            panic!()
        };
        assert_eq!(nome, "env");
        let mut g = edicao(serde_json::json!(["app.env"]));
        g["anexos"] = serde_json::json!([id]);
        let r = s.salvar_credencial(Some("gama-generico"), form(g)).unwrap();
        assert_eq!(r.salvo.as_deref(), Some("gama-generico"), "{r:?}");
        let nomes: Vec<String> = s
            .detalhe_credencial("gama-generico")
            .unwrap()
            .anexos
            .into_iter()
            .map(|a| a.nome)
            .collect();
        assert_eq!(nomes, ["app.env", "env"]);
        // Remover: o que não está em `anexos_mantidos` sai.
        let r = s
            .salvar_credencial(
                Some("gama-generico"),
                form(edicao(serde_json::json!(["env"]))),
            )
            .unwrap();
        assert_eq!(r.salvo.as_deref(), Some("gama-generico"), "{r:?}");
        let det = s.detalhe_credencial("gama-generico").unwrap();
        assert_eq!(det.anexos.len(), 1);
        assert_eq!(det.anexos[0].nome, "env");
        // Nome mantido desconhecido no `escolher_anexo` é pedido inválido.
        let forjado = EdicaoDosAnexos {
            entrada: "gama-generico".into(),
            mantidos: vec!["app.env".into()],
        };
        assert_eq!(
            s.escolher_anexo(Some(&forjado)),
            Err(ErroApp::PedidoInvalido)
        );
        // Nenhum quadro (pedido ou resposta) levou o conteúdo do `app.env`, em claro ou
        // em base64.
        // base64 de "FALSO_ENV=falso-anexo-01" (24 bytes: prefixo do base64 de qualquer
        // conteúdo que comece por ele).
        let b64 = "RkFMU09fRU5WPWZhbHNvLWFuZXhvLTAx";
        for q in quadros.lock().unwrap().iter() {
            let t = String::from_utf8_lossy(q);
            assert!(!t.contains(conteudo) && !t.contains(b64), "{t}");
            assert!(!t.contains("falso-anexo-01"), "{t}");
        }
        let _ = std::fs::remove_dir_all(d);
    }

    /// V5.24 do lado do app (D3): "Conectar" só para `servidor`; o token vai ao terminal
    /// e nunca à resposta; nome fora do formato nem chega ao daemon. (Reuso, prazo e
    /// entrada trocada são do daemon e da CLI, Fase 27.)
    #[test]
    fn ataque_conectar_so_servidor_e_sem_token_na_resposta() {
        let (s, _, ter) = servico_34();
        s.conectar("acme-ssh-salto").unwrap();
        let abertos = ter.0.lock().unwrap().clone();
        assert_eq!(abertos.len(), 1);
        assert_eq!(abertos[0].0, "acme-ssh-salto");
        assert_eq!(abertos[0].1, "Token(••••):32");
        assert_eq!(s.conectar("acme-pg-aprovacao"), Err(ErroApp::NaoEServidor));
        assert_eq!(s.conectar("nao-existe"), Err(ErroApp::EntradaInexistente));
        for nome in ["-oProxyCommand=x", "", "acme ssh", "Acme"] {
            assert_eq!(s.conectar(nome), Err(ErroApp::PedidoInvalido), "{nome}");
        }
        assert_eq!(ter.0.lock().unwrap().len(), 1);
        // Sem terminal montado, o erro fixo.
        let (sem, _) = servico();
        assert_eq!(
            sem.conectar("acme-ssh-salto"),
            Err(ErroApp::TerminalIndisponivel)
        );
        assert_eq!(sem.escolher_anexo(None), Err(ErroApp::SeletorIndisponivel));
    }
    /// Fase 39 (V5.23, S12): "Travar agora" limpa a área de transferência que ainda tem
    /// o valor copiado e o formulário, e o cofre fica travado: depois, revelar e copiar
    /// respondem "travado".
    #[test]
    fn ataque_travar_limpa_a_area_e_trava() {
        let (s, area) = servico();
        s.copiar_campo("acme-pg-dev", "senha").unwrap();
        assert!(area.0.lock().unwrap().is_some());
        assert_eq!(
            s.estado_da_bandeja(),
            crate::bandeja::EstadoBandeja::Destravado
        );
        s.travar().unwrap();
        assert!(area.0.lock().unwrap().is_none(), "área não foi limpa");
        assert_eq!(s.estado_do_cofre(), Ok(EstadoCofre::Travado));
        assert_eq!(
            s.estado_da_bandeja(),
            crate::bandeja::EstadoBandeja::Travado
        );
        assert_eq!(
            s.revelar_campo("acme-pg-dev", "senha").map(|_| ()),
            Err(ErroApp::Travado)
        );
        assert!(s.copiar_campo("acme-pg-dev", "senha").is_err());
        // Travar de novo não falha.
        s.travar().unwrap();
    }

    /// Daemon que anota a operação de cada pedido (para o teste da consulta da bandeja).
    struct Anotador(DaemonFalso, Arc<Mutex<Vec<String>>>);

    impl Daemon for Anotador {
        fn trocar(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
            let v: serde_json::Value = serde_json::from_slice(pedido).unwrap();
            self.1
                .lock()
                .unwrap()
                .push(v["op"].as_str().unwrap_or("?").to_owned());
            self.0.trocar(pedido)
        }
    }

    /// Ataque (Fase 39): a consulta periódica da bandeja não pode segurar o cofre aberto.
    /// Ela só manda `estado`, que o `wardd` não conta como uso
    /// (`wardd::trava::conta_como_uso`); nenhuma outra operação sai dela.
    #[test]
    fn ataque_consulta_da_bandeja_so_pede_estado() {
        let ops = Arc::new(Mutex::new(Vec::new()));
        let s = Servico::novo(
            Anotador(DaemonFalso::cheio(), Arc::clone(&ops)),
            Copiador::novo(Arc::new(Area::default())),
        );
        for _ in 0..5 {
            s.estado_da_bandeja();
        }
        assert_eq!(*ops.lock().unwrap(), vec!["estado"; 5]);
    }

    /// Sem daemon, travar ainda limpa o que é local e responde o erro.
    #[test]
    fn travar_sem_daemon_limpa_e_responde_erro() {
        struct Fora;
        impl Daemon for Fora {
            fn trocar(&mut self, _: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
                Err(ErroApp::DaemonIndisponivel)
            }
        }
        let area = Arc::new(Area::default());
        let s = Servico::novo(Fora, Copiador::novo(Arc::clone(&area)));
        *area.0.lock().unwrap() = Some(b"qualquer".to_vec());
        assert!(s.travar().is_err());
        assert_eq!(
            s.estado_da_bandeja(),
            crate::bandeja::EstadoBandeja::SemDaemon
        );
    }
}

/// Fase 31: destravar com a senha mestra e o pedido de destrave do agente.
#[cfg(test)]
mod testes_destravar {
    use super::*;
    use crate::daemon::{Aviso, FilaAvisos};
    use crate::daemon_falso::DaemonFalso;
    use std::time::Duration;
    use zeroize::Zeroizing;

    const SENHA_CERTA: &str = "falso-mestra-ward-01";

    struct SemArea;
    impl AreaTransferencia for SemArea {
        fn escrever(&self, _: &[u8]) -> Result<(), ErroApp> {
            Err(ErroApp::AreaTransferencia)
        }
        fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, ErroApp> {
            Ok(None)
        }
        fn limpar(&self) -> Result<(), ErroApp> {
            Ok(())
        }
    }

    fn servico() -> Servico<DaemonFalso, SemArea> {
        Servico::novo(DaemonFalso::travado().sem_espera(), Copiador::novo(SemArea))
    }

    fn senha(s: &str) -> SenhaMestra {
        SenhaMestra::nova(s.to_owned()).unwrap()
    }

    /// Guarda os quadros que o app manda e responde pelo daemon falso.
    struct Espiao(DaemonFalso, Arc<Mutex<Vec<Zeroizing<Vec<u8>>>>>);
    impl Daemon for Espiao {
        fn trocar(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
            self.1.lock().unwrap().push(Zeroizing::new(pedido.to_vec()));
            self.0.trocar(pedido)
        }
    }

    #[test]
    fn senha_errada_nao_destrava_e_a_certa_destrava() {
        let s = servico();
        assert_eq!(s.estado_do_cofre(), Ok(EstadoCofre::Travado));
        assert_eq!(s.listar_credenciais().unwrap_err(), ErroApp::Travado);
        for errada in [
            "falso-mestra-ward-02",
            "135790",
            "FALSO-MESTRA-WARD-01",
            " ",
        ] {
            assert_eq!(s.destravar(senha(errada)), Err(ErroApp::SenhaIncorreta));
            assert_eq!(s.estado_do_cofre(), Ok(EstadoCofre::Travado));
        }
        s.destravar(senha(SENHA_CERTA)).unwrap();
        assert_eq!(s.estado_do_cofre(), Ok(EstadoCofre::Destravado));
        assert!(!s.listar_credenciais().unwrap().is_empty());
        // Já destravado: `ok` de novo.
        s.destravar(senha("qualquer")).unwrap();
    }

    #[test]
    fn ataque_erro_do_destravar_sem_senha_nem_contagem() {
        // S13/D20: a mensagem do erro é fixa, sem a senha e sem quantas tentativas
        // houve (a 1ª e a 5ª dizem a mesma coisa).
        let s = servico();
        let primeira = s.destravar(senha("falso-mestra-ward-99")).unwrap_err();
        for _ in 0..4 {
            assert_eq!(
                s.destravar(senha("falso-mestra-ward-99")).unwrap_err(),
                primeira
            );
        }
        let texto = format!("{primeira} {primeira:?} {}", primeira.codigo());
        assert!(!texto.contains("falso"), "{texto}");
        assert!(!texto.chars().any(|c| c.is_ascii_digit()), "{texto}");
    }

    #[test]
    fn ataque_a_senha_so_vai_no_pedido_destravar() {
        // A senha sai do app só no quadro `destravar`; nenhum outro quadro a leva.
        let quadros = Arc::new(Mutex::new(Vec::new()));
        let s = Servico::novo(
            Espiao(DaemonFalso::travado().sem_espera(), quadros.clone()),
            Copiador::novo(SemArea),
        );
        let _ = s.estado_do_cofre();
        s.destravar(senha(SENHA_CERTA)).unwrap();
        let _ = s.listar_credenciais();
        let q = quadros.lock().unwrap();
        assert!(q.len() >= 3);
        let textos: Vec<String> = q
            .iter()
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .collect();
        assert_eq!(
            textos
                .iter()
                .filter(|t| t.contains("\"destravar\""))
                .count(),
            1
        );
        for t in textos.iter().filter(|t| !t.contains("\"destravar\"")) {
            assert!(!t.contains("falso-mestra"), "{t}");
            // base64 de "falso-mestra-ward-01" (como o protocolo leva a senha).
            assert!(!t.contains("ZmFsc28tbWVzdHJhLXdhcmQtMDE"), "{t}");
        }
    }

    #[test]
    fn pedido_do_agente_aparece_so_travado_e_some_ao_destravar() {
        let s = servico();
        let fila = Arc::new(FilaAvisos::default());
        let (tx, rx) = std::sync::mpsc::channel();
        crate::destrave::vigiar(fila.clone(), s.pedido_de_destrave(), move || {
            let _ = tx.send(());
        })
        .unwrap();
        assert_eq!(s.info_do_pedido(EstadoCofre::Travado), None);
        fila.simular(Aviso::destrave_de_teste("acme-pg-aprovacao"));
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let info = s.info_do_pedido(EstadoCofre::Travado).unwrap();
        assert!(info.restante > 110 && info.restante <= 120, "{info:?}");
        assert_eq!(s.info_do_pedido(EstadoCofre::Destravado), None);
        assert_eq!(s.info_do_pedido(EstadoCofre::SemCofre), None);
        // Senha errada não tira o pedido.
        let _ = s.destravar(senha("errada"));
        assert!(s.info_do_pedido(EstadoCofre::Travado).is_some());
        s.destravar(senha(SENHA_CERTA)).unwrap();
        assert_eq!(s.info_do_pedido(EstadoCofre::Travado), None);
        // O JSON que vai à tela: id, restante e o texto do pedido (Fase 39a), nada mais.
        fila.simular(Aviso::destrave_de_teste("acme-pg-aprovacao"));
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let j = serde_json::to_value(s.info_do_pedido(EstadoCofre::Travado).unwrap()).unwrap();
        let mut chaves: Vec<&String> = j.as_object().unwrap().keys().collect();
        chaves.sort();
        assert_eq!(chaves, ["comando", "entrada", "id", "operacao", "restante"]);
    }

    #[test]
    fn daemon_falso_espera_d20_antes_de_olhar_a_senha() {
        // A 2ª tentativa (depois de 1 erro) espera 1 s mesmo com a senha certa.
        let s = Servico::novo(DaemonFalso::travado(), Copiador::novo(SemArea));
        assert_eq!(s.destravar(senha("errada")), Err(ErroApp::SenhaIncorreta));
        let t = std::time::Instant::now();
        s.destravar(senha(SENHA_CERTA)).unwrap();
        assert!(
            t.elapsed() >= Duration::from_millis(990),
            "{:?}",
            t.elapsed()
        );
    }
}
