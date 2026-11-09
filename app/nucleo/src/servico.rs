//! O que cada comando do app faz. O Tauri (`app/src-tauri`) e a ponte de teste
//! (`ward-ponte-teste`) chamam só isto, então os dois têm o mesmo conjunto de comandos
//! e as mesmas respostas. Nenhuma resposta carrega segredo (S1, S11, D4).

use std::sync::Mutex;

use crate::ErroApp;
use crate::area_transferencia::{AreaTransferencia, Copiador};
use crate::comandos::Referencia;
use crate::daemon::{Daemon, EstadoCofre};
use crate::gerador::{Gerada, Gerador, Opcoes, Revelada};

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

    /// `estado_do_cofre`.
    pub fn estado_do_cofre(&self) -> Result<EstadoCofre, ErroApp> {
        self.daemon
            .lock()
            .map_err(|_| ErroApp::DaemonIndisponivel)?
            .estado()
    }

    /// `copiar_campo`: o valor vai do daemon direto para a área de transferência. A
    /// resposta é só "deu certo" ou o erro; o valor não volta.
    pub fn copiar_campo(&self, entrada: &str, campo: &str) -> Result<(), ErroApp> {
        let referencia = Referencia::nova(entrada, campo)?;
        let valor = self
            .daemon
            .lock()
            .map_err(|_| ErroApp::DaemonIndisponivel)?
            .valor_para_copiar(&referencia)?;
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
    use crate::segredo::Segredo;
    use std::sync::Arc;
    use zeroize::Zeroizing;

    struct DaemonFalso;

    impl Daemon for DaemonFalso {
        fn estado(&mut self) -> Result<EstadoCofre, ErroApp> {
            Ok(EstadoCofre::Destravado)
        }
        fn valor_para_copiar(&mut self, r: &Referencia) -> Result<Segredo, ErroApp> {
            match (r.entrada(), r.campo()) {
                ("acme-pg-dev", "senha") => Ok(Segredo::novo(b"falso-pg-dev-senha".to_vec())),
                _ => Err(ErroApp::PedidoInvalido),
            }
        }
    }

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

    #[test]
    fn ataque_copiar_nao_devolve_o_valor() {
        let area = Arc::new(Area::default());
        let s = Servico::novo(DaemonFalso, Copiador::novo(Arc::clone(&area)));
        // O tipo da resposta é `()`: não há valor para a interface ver.
        let resposta: Result<(), ErroApp> = s.copiar_campo("acme-pg-dev", "senha");
        assert_eq!(resposta, Ok(()));
        assert_eq!(format!("{resposta:?}"), "Ok(())");
        assert_eq!(
            area.0.lock().unwrap().as_deref(),
            Some(&b"falso-pg-dev-senha"[..])
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
        let s = Servico::novo(DaemonFalso, Copiador::novo(Arc::clone(&area)));
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
            DaemonFalso,
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
        let area = Arc::new(Area::default());
        let s = Servico::novo(DaemonFalso, Copiador::novo(Arc::clone(&area)));
        let e = s.copiar_campo("acme-pg-dev\n", "senha").unwrap_err();
        assert_eq!(e, ErroApp::PedidoInvalido);
        assert!(!e.to_string().contains("acme"));
        assert_eq!(s.estado_do_cofre(), Ok(EstadoCofre::Destravado));
    }
}
