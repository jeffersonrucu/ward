//! Cliente do socket do `wardd` (lado Rust do app; a webview nunca fala com o socket).
//!
//! Antes de mandar qualquer byte, o app confere o servidor pela mesma regra da CLI
//! (`ward_plataforma::conectar_servidor`, S5, D46): o socket é
//! `/run/ward/<uid>/wardd.sock`, toda pasta do caminho é do root, sem escrita para grupo
//! e outros e sem link, o socket é do próprio uid e 0600, e o `SO_PEERCRED` do par tem
//! uid 0 (quem fez o `listen` foi o systemd do sistema). Só no build de teste
//! (`modo-teste`, D7) o socket é o do daemon de teste, em `/run/user/<uid>/ward-teste/`
//! (pasta 0700 do usuário e par do mesmo uid); sem a feature, esse caminho e essa
//! exceção nem existem no binário, e nenhuma variável de ambiente escolhe o caminho.
//!
//! O protocolo (Fase 20) fica em [`crate::cliente`]: lá o pedido vira o corpo do
//! quadro e a resposta é lida no papel app. Este módulo é só o transporte.
//!
//! Ponto de ligação com a Fase 22 (`wardd`): [`DaemonSocket::trocar`] confere e abre o
//! socket e ainda devolve `ErroApp::SemProtocolo`; com o daemon pronto, o corpo passa a
//! ser `escrever_quadro(conexão, pedido, limite_do_quadro(Papel::App))` seguido de
//! `ler_quadro(conexão, LIMITE_RESPOSTA)` (`ward_core::protocolo::quadro`), mais o
//! tratamento dos avisos que o daemon manda sem pedido na mesma conexão
//! (`pedido_de_aprovacao`, `pedido_de_destrave`, Fase 23), que não podem ser lidos como
//! a resposta do pedido em curso.
//!
//! Conexão única (D47, Fase 29a): o app setgid conecta **uma vez**, antes do GTK, com o
//! egid do grupo `ward-app` (o papel vem dele), e depois larga o grupo
//! ([`crate::partida`]). Uma conexão nova depois disso teria o gid real e cairia no
//! papel agente; por isso o [`DaemonSocket`] da partida nunca reconecta: sem a
//! conexão, os pedidos respondem [`ErroApp::ConexaoPerdida`] e o app se executa de
//! novo. O [`DaemonSocket::padrao`] (ponte de teste, testes) conecta no primeiro uso.

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ward_plataforma::{RecusaServidor, ServidorEsperado, conectar_servidor};

use zeroize::Zeroizing;

use crate::ErroApp;

/// Nome do socket (o mesmo do `wardd`).
pub const SOCKET: &str = "wardd.sock";

/// Prazo de leitura e escrita no socket.
const PRAZO: Duration = Duration::from_secs(5);

/// Estado do cofre como a interface vê.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstadoCofre {
    SemCofre,
    Travado,
    Destravado,
}

impl EstadoCofre {
    pub fn codigo(self) -> &'static str {
        match self {
            EstadoCofre::SemCofre => "sem-cofre",
            EstadoCofre::Travado => "travado",
            EstadoCofre::Destravado => "destravado",
        }
    }
}

/// Transporte até o daemon. Recebe o corpo de um pedido já codificado no papel app e
/// devolve o corpo da resposta, ainda não lido (quem lê é [`crate::cliente`], com a
/// leitura estrita do protocolo). Os testes e a ponte de teste trocam o socket pelo
/// [`crate::daemon_falso`].
pub trait Daemon: Send {
    fn trocar(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp>;
}

/// Daemon real, pelo socket local.
pub struct DaemonSocket {
    esperado: ServidorEsperado,
    conexao: Option<UnixStream>,
    /// `false` depois da partida: a conexão do setgid não se refaz neste processo.
    pode_conectar: bool,
}

impl DaemonSocket {
    /// O daemon instalado: `/run/ward/<uid>/wardd.sock`, conferido como socket do
    /// systemd do sistema. No build de teste, o daemon de teste em
    /// `/run/user/<uid>/ward-teste/wardd.sock`. O uid é o efetivo do processo, não
    /// `XDG_RUNTIME_DIR` nem outra variável.
    pub fn padrao() -> Result<DaemonSocket, ErroApp> {
        #[cfg(not(feature = "modo-teste"))]
        let esperado = ServidorEsperado::sistema();
        #[cfg(feature = "modo-teste")]
        let esperado = ServidorEsperado::teste();
        Ok(DaemonSocket::sem_conexao(esperado))
    }

    fn sem_conexao(esperado: ServidorEsperado) -> DaemonSocket {
        DaemonSocket {
            esperado,
            conexao: None,
            pode_conectar: true,
        }
    }

    /// Conecta agora (com toda a conferência do servidor) e trava a conexão: daqui em
    /// diante este `DaemonSocket` nunca conecta de novo. É o que a partida do app chama
    /// antes de largar o grupo (D47).
    pub fn conectar_agora(&mut self) -> Result<&UnixStream, ErroApp> {
        self.pode_conectar = false;
        self.conexao = None;
        let s = self.conectar()?;
        Ok(self.conexao.insert(s))
    }

    /// A conexão aberta. Sem ela: conecta no primeiro uso, menos depois de
    /// [`conectar_agora`](Self::conectar_agora), quando responde
    /// [`ErroApp::ConexaoPerdida`].
    fn conexao(&mut self) -> Result<&UnixStream, ErroApp> {
        if self.conexao.is_none() {
            if !self.pode_conectar {
                return Err(ErroApp::ConexaoPerdida);
            }
            let s = self.conectar()?;
            self.conexao = Some(s);
        }
        self.conexao.as_ref().ok_or(ErroApp::ConexaoPerdida)
    }

    /// Outro caminho, com as regras do daemon instalado (pasta do root, par uid 0).
    pub fn sistema_em(caminho: PathBuf) -> DaemonSocket {
        DaemonSocket::sem_conexao(ServidorEsperado::sistema_em(caminho))
    }

    /// Outro caminho, com as regras do daemon de teste. Só no `modo-teste`.
    #[cfg(feature = "modo-teste")]
    pub fn teste_em(caminho: PathBuf) -> DaemonSocket {
        DaemonSocket::sem_conexao(ServidorEsperado::teste_em(caminho))
    }

    pub fn caminho(&self) -> &Path {
        self.esperado.caminho()
    }

    /// Confere o caminho, conecta e confere o par antes de qualquer byte. Nada é
    /// escrito no socket aqui.
    fn conectar(&self) -> Result<UnixStream, ErroApp> {
        let s = conectar_servidor(&self.esperado).map_err(|r| match r {
            RecusaServidor::Ausente | RecusaServidor::Io(_) => ErroApp::DaemonIndisponivel,
            _ => ErroApp::DaemonNaoConfiavel,
        })?;
        s.set_read_timeout(Some(PRAZO))
            .map_err(|_| ErroApp::DaemonIndisponivel)?;
        s.set_write_timeout(Some(PRAZO))
            .map_err(|_| ErroApp::DaemonIndisponivel)?;
        Ok(s)
    }
}

impl Daemon for DaemonSocket {
    fn trocar(&mut self, _pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
        let _conexao = self.conexao()?;
        // Fase 22: `escrever_quadro` + `ler_quadro` na conexão (ver o topo do módulo).
        // A resposta do `copiar` vem num quadro `Zeroizing` e vira `Segredo` direto.
        Err(ErroApp::SemProtocolo)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::fs;
    use std::io::Read;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;

    use crate::cliente;
    use crate::comandos::Referencia;
    use ward_core::protocolo::Alvo;

    impl DaemonSocket {
        fn estado(&mut self) -> Result<EstadoCofre, ErroApp> {
            cliente::estado(self)
        }
        fn valor_para_copiar(&mut self, r: &Referencia) -> Result<(), ErroApp> {
            cliente::copiar(self, r.entrada(), Alvo::Campo(r.campo().to_owned())).map(|_| ())
        }
    }

    fn meu_uid() -> u32 {
        fs::metadata("/proc/self").unwrap().uid()
    }

    /// Pasta só deste teste em `/run/user/<uid>/` (tmpfs, 0700 do usuário; o `/tmp` é
    /// 1777 e não passaria em conferência nenhuma). Apagada ao sair.
    struct Base(PathBuf);

    impl Base {
        fn nova(nome: &str) -> Base {
            let p = PathBuf::from(format!(
                "/run/user/{}/ward-app-nucleo-{}-{nome}",
                meu_uid(),
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
            Base(p)
        }
    }

    impl Drop for Base {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Servidor falso do mesmo uid: socket 0600 em `dir`.
    fn servidor_falso(dir: &Path) -> UnixListener {
        let l = UnixListener::bind(dir.join(SOCKET)).unwrap();
        fs::set_permissions(dir.join(SOCKET), fs::Permissions::from_mode(0o600)).unwrap();
        l.set_nonblocking(true).unwrap();
        l
    }

    /// Nenhum byte chegou ao servidor falso (aceita o que houver e lê sem bloquear).
    fn nada_recebido(l: &UnixListener) {
        while let Ok((mut c, _)) = l.accept() {
            c.set_nonblocking(true).unwrap();
            let mut buf = [0u8; 16];
            assert!(
                !matches!(c.read(&mut buf), Ok(n) if n > 0),
                "o app mandou bytes a um servidor recusado"
            );
        }
    }

    #[test]
    fn caminho_padrao_sem_variavel_de_ambiente() {
        let d = DaemonSocket::padrao().unwrap();
        #[cfg(not(feature = "modo-teste"))]
        let esperado = format!("/run/ward/{}/wardd.sock", meu_uid());
        #[cfg(feature = "modo-teste")]
        let esperado = format!("/run/user/{}/ward-teste/wardd.sock", meu_uid());
        assert_eq!(d.caminho(), Path::new(&esperado));
        #[cfg(not(feature = "modo-teste"))]
        assert!(!d.caminho().to_string_lossy().contains("ward-teste"));
    }

    #[test]
    fn socket_ausente_e_indisponivel() {
        let mut d =
            DaemonSocket::sistema_em(PathBuf::from("/run/ward-nao-existe-falso/1/wardd.sock"));
        assert_eq!(d.estado(), Err(ErroApp::DaemonIndisponivel));
        #[cfg(feature = "modo-teste")]
        {
            let base = Base::nova("ausente");
            let dir = base.0.join("ward-teste");
            fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
            let mut t = DaemonSocket::teste_em(dir.join(SOCKET));
            assert_eq!(t.estado(), Err(ErroApp::DaemonIndisponivel));
        }
    }

    #[test]
    fn ataque_servidor_falso_do_mesmo_uid_e_recusado_sem_receber_nada() {
        // V3.23: servidor do mesmo uid, socket 0600, pasta 0700 do usuário. Pela regra
        // do daemon instalado, a pasta não é do root e o par não é uid 0.
        let base = Base::nova("falso");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let l = servidor_falso(&dir);
        let mut d = DaemonSocket::sistema_em(dir.join(SOCKET));
        assert_eq!(d.estado(), Err(ErroApp::DaemonNaoConfiavel));
        let r = Referencia::nova("acme-pg-dev", "senha").unwrap();
        assert!(matches!(
            d.valor_para_copiar(&r),
            Err(ErroApp::DaemonNaoConfiavel)
        ));
        nada_recebido(&l);
    }

    #[test]
    fn ataque_sem_a_conexao_da_partida_nao_reconecta() {
        // D47: depois de largar o grupo, uma conexão nova teria o gid real (papel
        // agente). Falhou na partida: nenhum pedido conecta depois.
        let base = Base::nova("partida");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let mut d = DaemonSocket::sistema_em(dir.join(SOCKET));
        assert!(d.conectar_agora().is_err());
        let l = servidor_falso(&dir);
        assert_eq!(d.estado(), Err(ErroApp::ConexaoPerdida));
        let r = Referencia::nova("acme-pg-dev", "senha").unwrap();
        assert!(matches!(
            d.valor_para_copiar(&r),
            Err(ErroApp::ConexaoPerdida)
        ));
        assert!(l.accept().is_err(), "reconectou depois da partida");
        #[cfg(feature = "modo-teste")]
        {
            // Com a conexão da partida aberta, os pedidos usam a mesma, sem conectar
            // de novo.
            let mut t = DaemonSocket::teste_em(dir.join(SOCKET));
            let c = t.conectar_agora().unwrap();
            assert!(ward_plataforma::app::tem_cloexec(c).unwrap());
            assert!(l.accept().is_ok());
            assert_eq!(t.estado(), Err(ErroApp::SemProtocolo));
            assert_eq!(t.estado(), Err(ErroApp::SemProtocolo));
            assert!(l.accept().is_err(), "conectou de novo");
        }
    }

    #[test]
    fn ataque_diretorio_aberto_para_outros_e_recusado() {
        let base = Base::nova("aberto");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o755).create(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        let l = servidor_falso(&dir);
        let mut d = DaemonSocket::sistema_em(dir.join(SOCKET));
        assert_eq!(d.estado(), Err(ErroApp::DaemonNaoConfiavel));
        #[cfg(feature = "modo-teste")]
        {
            // Nem o build de teste aceita pasta aberta; com 0700 é o daemon de teste.
            let mut t = DaemonSocket::teste_em(dir.join(SOCKET));
            assert_eq!(t.estado(), Err(ErroApp::DaemonNaoConfiavel));
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
            assert_eq!(t.estado(), Err(ErroApp::SemProtocolo));
        }
        nada_recebido(&l);
    }

    #[test]
    fn ataque_diretorio_por_link_simbolico_e_recusado() {
        let base = Base::nova("link");
        let real = base.0.join("real");
        fs::DirBuilder::new().mode(0o700).create(&real).unwrap();
        let l = servidor_falso(&real);
        symlink(&real, base.0.join("ward")).unwrap();
        let mut d = DaemonSocket::sistema_em(base.0.join("ward").join(SOCKET));
        assert_eq!(d.estado(), Err(ErroApp::DaemonNaoConfiavel));
        #[cfg(feature = "modo-teste")]
        {
            let mut t = DaemonSocket::teste_em(base.0.join("ward").join(SOCKET));
            assert_eq!(t.estado(), Err(ErroApp::DaemonNaoConfiavel));
        }
        nada_recebido(&l);
    }

    #[test]
    fn ataque_arquivo_comum_no_lugar_do_socket_e_recusado() {
        let base = Base::nova("arquivo");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        fs::write(dir.join(SOCKET), b"").unwrap();
        fs::set_permissions(dir.join(SOCKET), fs::Permissions::from_mode(0o600)).unwrap();
        let mut d = DaemonSocket::sistema_em(dir.join(SOCKET));
        assert_eq!(d.estado(), Err(ErroApp::DaemonNaoConfiavel));
        #[cfg(feature = "modo-teste")]
        {
            let mut t = DaemonSocket::teste_em(dir.join(SOCKET));
            assert_eq!(t.estado(), Err(ErroApp::DaemonNaoConfiavel));
        }
    }

    #[test]
    fn ataque_socket_aberto_para_outros_e_recusado() {
        let base = Base::nova("socket");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let l = servidor_falso(&dir);
        fs::set_permissions(dir.join(SOCKET), fs::Permissions::from_mode(0o666)).unwrap();
        let mut d = DaemonSocket::sistema_em(dir.join(SOCKET));
        assert_eq!(d.estado(), Err(ErroApp::DaemonNaoConfiavel));
        #[cfg(feature = "modo-teste")]
        {
            let mut t = DaemonSocket::teste_em(dir.join(SOCKET));
            assert_eq!(t.estado(), Err(ErroApp::DaemonNaoConfiavel));
        }
        nada_recebido(&l);
    }
}
