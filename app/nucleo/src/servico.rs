//! O que cada comando do app faz. O Tauri (`app/src-tauri`) e a ponte de teste
//! (`ward-ponte-teste`) chamam só isto, então os dois têm o mesmo conjunto de comandos
//! e as mesmas respostas. Nenhuma resposta carrega segredo (S1, S11, D4).

use std::sync::{Mutex, MutexGuard};

use ward_core::protocolo::Alvo;

use crate::ErroApp;
use crate::area_transferencia::{AreaTransferencia, Copiador};
use crate::cliente;
use crate::comandos::{Referencia, validar_nome_entrada};
use crate::credenciais::{DetalheCredencial, ItemCredencial};
use crate::daemon::{Daemon, EstadoCofre};
use crate::gerador::{Gerada, Gerador, Opcoes, Revelada};
use crate::segredo::Segredo;

pub struct Servico<D: Daemon, A: AreaTransferencia> {
    daemon: Mutex<D>,
    copiador: Copiador<A>,
    /// Senha avulsa da tela `Gerador de senha` (Fase 37), só em memória.
    gerador: Mutex<Gerador>,
}

impl<D: Daemon, A: AreaTransferencia> Servico<D, A> {
    pub fn novo(daemon: D, copiador: Copiador<A>) -> Self {
        Servico {
            daemon: Mutex::new(daemon),
            copiador,
            gerador: Mutex::new(Gerador::default()),
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
        if let Ok(mut g) = self.gerador_travado() {
            g.descartar();
        }
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

    #[test]
    fn cofre_vazio() {
        let area = Arc::new(Area::default());
        let s = Servico::novo(DaemonFalso::vazio(), Copiador::novo(area));
        assert!(s.listar_credenciais().unwrap().is_empty());
    }
}
