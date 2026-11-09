//! O que cada comando do app faz. O Tauri (`app/src-tauri`) e a ponte de teste
//! (`ward-ponte-teste`) chamam só isto, então os dois têm o mesmo conjunto de comandos
//! e as mesmas respostas. Nenhuma resposta carrega segredo (S1, S11, D4).

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use serde::Serialize;
use ward_core::entrada::{Rascunho, Segredo as SegredoCore};
use ward_core::protocolo::Alvo;

use crate::ErroApp;
use crate::area_transferencia::{AreaTransferencia, Copiador};
use crate::cliente;
use crate::comandos::{Referencia, validar_nome_entrada};
use crate::credenciais::{DetalheCredencial, ItemCredencial};
use crate::daemon::{Daemon, EstadoCofre};
use crate::formulario::{self, ErroCampo, Falta, Fontes, Formulario};
use crate::gerador::{Gerada, Gerador, Opcoes, Revelada};
use crate::nivel::{NivelMudado, Pin, nivel_de_nome};
use crate::segredo::Segredo;

/// Campos do formulário que aceitam "Usar esta senha" (o `Campo secreto` com gerar no
/// design é só o da senha).
const CAMPOS_GERAVEIS: &[&str] = &["senha"];

/// Resposta do `salvar_credencial`: o nome salvo, ou os erros por campo. Sem valor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResultadoSalvar {
    pub salvo: Option<String>,
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
}

/// Valores que o formulário pede sem passar pela interface.
struct FontesDoServico<'a, D: Daemon> {
    daemon: &'a mut D,
    entrada: Option<&'a str>,
    usadas: &'a BTreeMap<String, Segredo>,
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
}

impl<D: Daemon, A: AreaTransferencia> Servico<D, A> {
    pub fn novo(daemon: D, copiador: Copiador<A>) -> Self {
        Servico {
            daemon: Mutex::new(daemon),
            copiador,
            gerador: Mutex::new(Gerador::default()),
            usadas: Mutex::new(BTreeMap::new()),
        }
    }

    fn daemon(&self) -> Result<MutexGuard<'_, D>, ErroApp> {
        self.daemon.lock().map_err(|_| ErroApp::DaemonIndisponivel)
    }

    /// `estado_do_cofre`.
    pub fn estado_do_cofre(&self) -> Result<EstadoCofre, ErroApp> {
        cliente::estado(&mut *self.daemon()?)
    }

    /// `listar_credenciais`: `lista` e, para a segunda linha de cada item (usuário,
    /// porta, motor), o `detalhe` de cada entrada. Nada de segredo.
    pub fn listar_credenciais(&self) -> Result<Vec<ItemCredencial>, ErroApp> {
        let mut d = self.daemon()?;
        let itens = cliente::lista(&mut *d)?;
        itens
            .iter()
            .map(|item| {
                let detalhe = cliente::detalhe(&mut *d, &item.nome)?;
                Ok(ItemCredencial::novo(item, &detalhe))
            })
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
                if !det.anexos.is_empty() {
                    return Err(ErroApp::AnexosNaoMantidos);
                }
                Some(det)
            }
            None => None,
        };
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
        let mut fontes = FontesDoServico {
            daemon: &mut *d,
            entrada: anterior,
            usadas: &usadas,
        };
        let montado = match formulario::montar(&mut f, tipo, editando, &mut fontes) {
            Ok(m) => m,
            Err(Falta::App(e)) => return Err(e),
            Err(Falta::Campos(erros)) => {
                return Ok(ResultadoSalvar { salvo: None, erros });
            }
        };
        drop(usadas);
        let nome = f.nome.clone();
        let rascunho = Rascunho {
            nome: f.nome,
            grupo: f.grupo,
            ambiente: f.ambiente,
            projeto: f.projeto,
            tags: f.tags,
            dados: montado.dados,
            extras: montado.extras,
            anexos: Vec::new(),
        };
        if let Err(erros) = formulario::validar_local(&rascunho) {
            return Ok(ResultadoSalvar { salvo: None, erros });
        }
        match cliente::salvar(&mut *d, anterior.map(str::to_owned), rascunho)? {
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
        // Tipo da Fase 34 e tipo inventado.
        for tipo in ["banco", "servidor", "ftp-sftp", "root", ""] {
            let mut c = login_novo("acme-x");
            c["tipo"] = serde_json::json!(tipo);
            assert_eq!(
                s.salvar_credencial(None, form(c)),
                Err(ErroApp::PedidoInvalido),
                "{tipo}"
            );
        }
        // Anexos.
        let g = serde_json::json!({
            "tipo": "generico", "nome": "gama-generico", "grupo": "gama", "ambiente": "stage",
            "projeto": "", "tags": [], "campos": {}, "segredos": {},
            "extras": [{"nome": "REGIAO", "segredo": false, "valor": {"novo": "r-02"}}]
        });
        assert_eq!(
            s.salvar_credencial(Some("gama-generico"), form(g)),
            Err(ErroApp::AnexosNaoMantidos)
        );
        assert_eq!(
            s.detalhe_credencial("gama-generico").unwrap().anexos.len(),
            1
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
}
