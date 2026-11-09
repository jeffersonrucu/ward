//! O que cada comando do app faz. O Tauri (`app/src-tauri`) e a ponte de teste
//! (`ward-ponte-teste`) chamam só isto, então os dois têm o mesmo conjunto de comandos
//! e as mesmas respostas. Nenhuma resposta carrega segredo (S1, S11, D4).

use std::sync::Mutex;

use crate::ErroApp;
use crate::area_transferencia::{AreaTransferencia, Copiador};
use crate::comandos::Referencia;
use crate::daemon::{Daemon, EstadoCofre};

pub struct Servico<D: Daemon, A: AreaTransferencia> {
    daemon: Mutex<D>,
    copiador: Copiador<A>,
}

impl<D: Daemon, A: AreaTransferencia> Servico<D, A> {
    pub fn novo(daemon: D, copiador: Copiador<A>) -> Self {
        Servico {
            daemon: Mutex::new(daemon),
            copiador,
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

    /// Ao travar ou fechar o app: limpa a área se ainda tiver o valor copiado.
    pub fn ao_sair(&self) {
        self.copiador.limpar_agora();
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
