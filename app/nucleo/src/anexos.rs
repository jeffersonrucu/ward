//! Anexos do formulário (Fase 34, V5.12; D29, D32).
//!
//! S11: o conteúdo de um anexo nunca passa pela webview. A interface só pede
//! `escolher_anexo`; quem escolhe o arquivo é o [`Seletor`] do lado Rust (no app, o
//! diálogo de arquivo do GTK; na ponte de teste, uma pasta fixa dada na linha de
//! comando). O arquivo é lido aqui, guardado em [`Segredo`] (zera ao sair) e a resposta
//! traz só o identificador, o nome e o tamanho.
//!
//! Os limites saem de `ward_core::limites` (D32): um arquivo acima de
//! `MAX_ANEXOS_SOMADOS`, ou que faria a soma passar disso, gera um aviso e não é
//! guardado (nem enviado ao daemon). Arquivo vazio é recusado (D29).
//!
//! Fase 39b (V5.12): na edição, a soma e a contagem partem dos anexos já guardados que
//! continuam na entrada ([`Base`]); o conteúdo deles fica no daemon. Nome de arquivo
//! começando por `.` (ex.: `.env`) é anexado sem os pontos do começo (`env`): o
//! `ward-core` não aceita nome de anexo que comece por `.` (o nome vira arquivo em
//! `/run/user/<uid>/ward/tmp/` e `WARD_ARQUIVO_<NOME>`), e a tela mostra o nome guardado.

use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ward_core::limites;

use crate::ErroApp;
use crate::segredo::Segredo;

/// Quem escolhe o arquivo. `Ok(None)`: o usuário cancelou.
pub trait Seletor: Send + Sync {
    fn escolher(&self) -> Result<Option<PathBuf>, ErroApp>;

    /// O arquivo da chave privada ("Carregar de arquivo", Fase 53a). O padrão é o mesmo
    /// seletor; o do GTK troca só o título e o botão do diálogo.
    fn escolher_chave(&self) -> Result<Option<PathBuf>, ErroApp> {
        self.escolher()
    }
}

/// Sem seletor (o padrão do serviço até alguém montar um).
pub struct SemSeletor;

impl Seletor for SemSeletor {
    fn escolher(&self) -> Result<Option<PathBuf>, ErroApp> {
        Err(ErroApp::SeletorIndisponivel)
    }
}

/// Um anexo escolhido e ainda não salvo.
pub struct Pendente {
    pub id: u64,
    pub nome: String,
    pub conteudo: Segredo,
}

/// O que a interface recebe do `escolher_anexo`. Nunca o conteúdo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "resultado", rename_all = "kebab-case")]
pub enum Escolha {
    Anexado {
        id: u64,
        nome: String,
        tamanho: u64,
    },
    Cancelado,
    /// O arquivo não foi guardado; o motivo é um texto fixo.
    Aviso {
        motivo: String,
    },
}

/// Textos dos avisos (fixos; nenhum caminho nem erro do sistema, S13).
pub fn aviso_grande() -> String {
    format!(
        "o arquivo passa de {} KiB e não foi anexado",
        limites::MAX_ANEXOS_SOMADOS / limites::KIB
    )
}

pub fn aviso_soma() -> String {
    format!(
        "com este arquivo os anexos somariam mais de {} KiB; ele não foi anexado",
        limites::MAX_ANEXOS_SOMADOS / limites::KIB
    )
}

pub const AVISO_VAZIO: &str = "arquivo vazio: anexo vazio não é aceito";
pub const AVISO_DEMAIS: &str = "anexos demais nesta entrada";
pub const AVISO_LEITURA: &str = "não foi possível ler o arquivo";
pub const AVISO_NOME: &str = "o nome do arquivo não é texto válido";

/// O que já conta para os limites antes dos escolhidos agora: na edição, os anexos
/// guardados que continuam na entrada (tamanho somado e quantidade). Na entrada nova,
/// zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Base {
    pub soma: usize,
    pub quantidade: usize,
}

/// Argumento do `escolher_anexo` na edição (Fase 39b): a entrada editada e os nomes
/// dos anexos guardados que a tela ainda mantém. Não confiável: o serviço confere os
/// nomes no detalhe da entrada e tira deles a [`Base`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdicaoDosAnexos {
    pub entrada: String,
    pub mantidos: Vec<String>,
}

/// Nome com que o arquivo é anexado: o do arquivo sem os `.` do começo (Fase 39b; o
/// `ward-core` recusa nome de anexo que comece por `.`). `None` se não sobrar nada.
pub fn nome_do_anexo(arquivo: &str) -> Option<&str> {
    let nome = arquivo.trim_start_matches('.');
    (!nome.is_empty()).then_some(nome)
}

/// Os anexos escolhidos no formulário aberto.
#[derive(Default)]
pub struct Anexos {
    pendentes: Vec<Pendente>,
    proximo: u64,
}

impl Anexos {
    /// Soma dos tamanhos guardados.
    pub fn soma(&self) -> usize {
        self.pendentes.iter().map(|p| p.conteudo.len()).sum()
    }

    /// Lê o arquivo escolhido e o guarda, ou devolve o aviso (entrada nova).
    pub fn anexar(&mut self, caminho: &Path) -> Escolha {
        self.anexar_sobre(caminho, Base::default())
    }

    /// Como [`Anexos::anexar`], com os limites contados a partir de `base` (os anexos
    /// guardados que continuam na entrada editada).
    pub fn anexar_sobre(&mut self, caminho: &Path, base: Base) -> Escolha {
        let aviso = |m: String| Escolha::Aviso { motivo: m };
        if self.pendentes.len().saturating_add(base.quantidade) >= limites::MAX_ANEXOS {
            return aviso(AVISO_DEMAIS.to_owned());
        }
        let Some(nome) = caminho
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(nome_do_anexo)
        else {
            return aviso(AVISO_NOME.to_owned());
        };
        if nome.is_empty() || nome.len() > limites::MAX_NOME_ANEXO * 4 {
            return aviso(AVISO_NOME.to_owned());
        }
        let nome = nome.to_owned();
        let livre = limites::MAX_ANEXOS_SOMADOS.saturating_sub(base.soma);
        let conteudo = match ler_limitado(caminho, limites::MAX_ANEXOS_SOMADOS) {
            Ok(Leitura::Conteudo(c)) => c,
            Ok(Leitura::Grande) => return aviso(aviso_grande()),
            Err(()) => return aviso(AVISO_LEITURA.to_owned()),
        };
        if conteudo.is_empty() {
            return aviso(AVISO_VAZIO.to_owned());
        }
        if self.soma().saturating_add(conteudo.len()) > livre {
            return aviso(aviso_soma());
        }
        let id = self.proximo;
        self.proximo += 1;
        let tamanho = conteudo.len() as u64;
        self.pendentes.push(Pendente {
            id,
            nome: nome.clone(),
            conteudo,
        });
        Escolha::Anexado { id, nome, tamanho }
    }

    /// Tira um anexo escolhido. Identificador desconhecido é pedido inválido.
    pub fn remover(&mut self, id: u64) -> Result<(), ErroApp> {
        let i = self
            .pendentes
            .iter()
            .position(|p| p.id == id)
            .ok_or(ErroApp::PedidoInvalido)?;
        drop(self.pendentes.remove(i));
        Ok(())
    }

    /// Os anexos pedidos pelo formulário, na ordem dele. Cada id uma vez só e só os
    /// que existem (a interface não é confiável).
    pub fn escolhidos(&self, ids: &[u64]) -> Result<Vec<&Pendente>, ErroApp> {
        let mut saida: Vec<&Pendente> = Vec::with_capacity(ids.len());
        for id in ids {
            if saida.iter().any(|p| p.id == *id) {
                return Err(ErroApp::PedidoInvalido);
            }
            saida.push(
                self.pendentes
                    .iter()
                    .find(|p| p.id == *id)
                    .ok_or(ErroApp::PedidoInvalido)?,
            );
        }
        Ok(saida)
    }

    /// Zera tudo (cancelar, salvar, sair).
    pub fn limpar(&mut self) {
        self.pendentes.clear();
    }

    pub fn len(&self) -> usize {
        self.pendentes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pendentes.is_empty()
    }
}

enum Leitura {
    Conteudo(Segredo),
    Grande,
}

/// Lê no máximo `max` bytes de um arquivo regular. O tamanho é conferido pelo `fstat`
/// do arquivo aberto e de novo pelo que foi lido (o arquivo pode crescer entre os dois).
///
/// Fase 39f (auditoria do M5), como o `chave.rs`: `O_NOFOLLOW` recusa link simbólico no
/// último componente (o usuário escolhe o arquivo, não um atalho para outro lugar) e
/// `O_NONBLOCK` faz uma FIFO não prender o app na abertura (ela é recusada pelo `fstat`
/// logo depois); `O_NOCTTY`, para um terminal escolhido não virar o terminal do app.
fn ler_limitado(caminho: &Path, max: usize) -> Result<Leitura, ()> {
    let arquivo = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(caminho)
        .map_err(|_| ())?;
    let meta = arquivo.metadata().map_err(|_| ())?;
    if !meta.is_file() {
        return Err(());
    }
    if meta.len() > max as u64 {
        return Ok(Leitura::Grande);
    }
    let mut buf = zeroize::Zeroizing::new(Vec::with_capacity(meta.len() as usize + 1));
    arquivo
        .take(max as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|_| ())?;
    if buf.len() > max {
        return Ok(Leitura::Grande);
    }
    Ok(Leitura::Conteudo(Segredo::novo(core::mem::take(&mut *buf))))
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn pasta(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ward-anexos-{}-{nome}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn arquivo(dir: &Path, nome: &str, tamanho: usize) -> PathBuf {
        let p = dir.join(nome);
        let mut f = File::create(&p).unwrap();
        f.write_all(&vec![b'x'; tamanho]).unwrap();
        p
    }

    #[test]
    fn anexa_lista_e_remove_sem_conteudo_na_resposta() {
        let d = pasta("ok");
        let mut a = Anexos::default();
        let e = a.anexar(&arquivo(&d, "deploy.pem", 1200));
        assert_eq!(
            e,
            Escolha::Anexado {
                id: 0,
                nome: "deploy.pem".into(),
                tamanho: 1200
            }
        );
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("xxxxxx"));
        assert_eq!(a.escolhidos(&[0]).unwrap().len(), 1);
        a.remover(0).unwrap();
        assert!(a.is_empty());
        assert_eq!(a.remover(0), Err(ErroApp::PedidoInvalido));
        let _ = std::fs::remove_dir_all(d);
    }

    /// V5.12: 300 KiB avisa e não guarda; passar de 256 KiB somados avisa e não guarda;
    /// arquivo vazio é recusado; diretório e arquivo inexistente dão aviso fixo.
    /// Fase 39f (baixo da auditoria do M5): link simbólico escolhido como anexo é
    /// recusado (não segue para o alvo), e uma FIFO não prende o app: volta o aviso
    /// fixo na hora, sem guardar nada.
    #[test]
    fn ataque_link_e_fifo_sao_recusados_sem_prender() {
        let d = pasta("link-fifo");
        let alvo = arquivo(&d, "alvo.txt", 64);
        let link = d.join("atalho.txt");
        std::os::unix::fs::symlink(&alvo, &link).unwrap();
        let mut a = Anexos::default();
        assert_eq!(
            a.anexar(&link),
            Escolha::Aviso {
                motivo: AVISO_LEITURA.into()
            }
        );
        let fifo = d.join("cano");
        let ok = std::process::Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(ok.success());
        let (tx, rx) = std::sync::mpsc::channel();
        let f = fifo.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Anexos::default().anexar(&f));
        });
        let r = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("a FIFO prendeu a leitura do anexo");
        assert_eq!(
            r,
            Escolha::Aviso {
                motivo: AVISO_LEITURA.into()
            }
        );
        assert!(a.is_empty());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn ataque_arquivo_grande_soma_e_vazio_nao_sao_guardados() {
        let d = pasta("limites");
        let mut a = Anexos::default();
        assert_eq!(
            a.anexar(&arquivo(&d, "grande.bin", 300 * limites::KIB)),
            Escolha::Aviso {
                motivo: aviso_grande()
            }
        );
        assert!(a.is_empty());
        assert!(matches!(
            a.anexar(&arquivo(&d, "a.bin", 200 * limites::KIB)),
            Escolha::Anexado { .. }
        ));
        assert_eq!(
            a.anexar(&arquivo(&d, "b.bin", 100 * limites::KIB)),
            Escolha::Aviso {
                motivo: aviso_soma()
            }
        );
        assert_eq!(
            a.anexar(&arquivo(&d, "vazio.txt", 0)),
            Escolha::Aviso {
                motivo: AVISO_VAZIO.into()
            }
        );
        assert_eq!(a.len(), 1);
        assert_eq!(a.soma(), 200 * limites::KIB);
        assert_eq!(
            a.anexar(&d),
            Escolha::Aviso {
                motivo: AVISO_LEITURA.into()
            }
        );
        assert_eq!(
            a.anexar(&d.join("nao-existe")),
            Escolha::Aviso {
                motivo: AVISO_LEITURA.into()
            }
        );
        // Exatamente no limite somado passa.
        assert!(matches!(
            a.anexar(&arquivo(&d, "c.bin", 56 * limites::KIB)),
            Escolha::Anexado { .. }
        ));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn ataque_ids_forjados_pela_interface() {
        let d = pasta("ids");
        let mut a = Anexos::default();
        a.anexar(&arquivo(&d, "x.pem", 10));
        assert_eq!(a.escolhidos(&[0, 0]).err(), Some(ErroApp::PedidoInvalido));
        assert_eq!(a.escolhidos(&[7]).err(), Some(ErroApp::PedidoInvalido));
        assert!(a.escolhidos(&[]).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(d);
    }

    /// Fase 39b (V5.12): na edição, os anexos guardados que continuam contam na soma e
    /// na quantidade; um arquivo que só caberia sem eles avisa e não é guardado.
    #[test]
    fn ataque_soma_com_os_anexos_mantidos() {
        let d = pasta("base");
        let mut a = Anexos::default();
        let base = Base {
            soma: 200 * limites::KIB,
            quantidade: 1,
        };
        assert_eq!(
            a.anexar_sobre(&arquivo(&d, "b.bin", 100 * limites::KIB), base),
            Escolha::Aviso {
                motivo: aviso_soma()
            }
        );
        assert!(a.is_empty());
        assert!(matches!(
            a.anexar_sobre(&arquivo(&d, "c.bin", 56 * limites::KIB), base),
            Escolha::Anexado { .. }
        ));
        let cheia = Base {
            soma: 0,
            quantidade: limites::MAX_ANEXOS,
        };
        assert_eq!(
            a.anexar_sobre(&arquivo(&d, "d.bin", 1), cheia),
            Escolha::Aviso {
                motivo: AVISO_DEMAIS.into()
            }
        );
        let _ = std::fs::remove_dir_all(d);
    }

    /// Fase 39b: `.env` é anexado como `env` (o `ward-core` recusa `.` no começo); um
    /// nome só de pontos avisa. O nome guardado nunca começa por `.`.
    #[test]
    fn ataque_nome_com_ponto_no_comeco() {
        let d = pasta("ponto");
        let mut a = Anexos::default();
        let e = a.anexar(&arquivo(&d, ".env", 12));
        assert!(
            matches!(&e, Escolha::Anexado { nome, .. } if nome == "env"),
            "{e:?}"
        );
        let e = a.anexar(&arquivo(&d, "..kube.cfg", 3));
        assert!(
            matches!(&e, Escolha::Anexado { nome, .. } if nome == "kube.cfg"),
            "{e:?}"
        );
        assert_eq!(nome_do_anexo("..."), None);
        assert_eq!(nome_do_anexo("."), None);
        assert_eq!(nome_do_anexo(".."), None);
        assert_eq!(nome_do_anexo("a.pem"), Some("a.pem"));
        assert!(
            a.escolhidos(&[0, 1])
                .unwrap()
                .iter()
                .all(|p| !p.nome.starts_with('.'))
        );
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn ataque_anexos_demais() {
        let d = pasta("demais");
        let mut a = Anexos::default();
        let p = arquivo(&d, "um.txt", 1);
        for _ in 0..limites::MAX_ANEXOS {
            assert!(matches!(a.anexar(&p), Escolha::Anexado { .. }));
        }
        assert_eq!(
            a.anexar(&p),
            Escolha::Aviso {
                motivo: AVISO_DEMAIS.into()
            }
        );
        let _ = std::fs::remove_dir_all(d);
    }
}
