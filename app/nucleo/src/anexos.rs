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

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use ward_core::limites;

use crate::ErroApp;
use crate::segredo::Segredo;

/// Quem escolhe o arquivo. `Ok(None)`: o usuário cancelou.
pub trait Seletor: Send + Sync {
    fn escolher(&self) -> Result<Option<PathBuf>, ErroApp>;
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

    /// Lê o arquivo escolhido e o guarda, ou devolve o aviso.
    pub fn anexar(&mut self, caminho: &Path) -> Escolha {
        let aviso = |m: String| Escolha::Aviso { motivo: m };
        if self.pendentes.len() >= limites::MAX_ANEXOS {
            return aviso(AVISO_DEMAIS.to_owned());
        }
        let Some(nome) = caminho.file_name().and_then(|n| n.to_str()) else {
            return aviso(AVISO_NOME.to_owned());
        };
        if nome.is_empty() || nome.len() > limites::MAX_NOME_ANEXO * 4 {
            return aviso(AVISO_NOME.to_owned());
        }
        let nome = nome.to_owned();
        let conteudo = match ler_limitado(caminho, limites::MAX_ANEXOS_SOMADOS) {
            Ok(Leitura::Conteudo(c)) => c,
            Ok(Leitura::Grande) => return aviso(aviso_grande()),
            Err(()) => return aviso(AVISO_LEITURA.to_owned()),
        };
        if conteudo.is_empty() {
            return aviso(AVISO_VAZIO.to_owned());
        }
        if self.soma().saturating_add(conteudo.len()) > limites::MAX_ANEXOS_SOMADOS {
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
fn ler_limitado(caminho: &Path, max: usize) -> Result<Leitura, ()> {
    let arquivo = File::open(caminho).map_err(|_| ())?;
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
