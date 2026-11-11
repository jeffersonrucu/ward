//! Leitura estrita dos objetos do quadro: cada campo é tirado pelo nome, com tipo e
//! tamanho conferidos, e [`Campos::fim`] recusa o que sobrou. Um campo inventado
//! (`"revelar": true`, `"geracao": 7`) recusa o pedido inteiro.

use zeroize::Zeroizing;

use super::json::{JsonInvalido, Valor, de_base64};
use crate::entry::Secret;

pub(crate) type R<T> = Result<T, JsonInvalido>;

/// Os pares de um objeto ainda não lidos.
pub(crate) struct Campos(Vec<(String, Valor)>);

impl Campos {
    /// O objeto, recusando chave repetida (dois valores para o mesmo campo
    /// permitiriam ler um e conferir outro).
    pub(crate) fn de(v: Valor) -> R<Self> {
        let c = Self::sem_conferir(v)?;
        c.sem_repetidas()?;
        Ok(c)
    }

    /// O objeto sem conferir as chaves ainda (o topo do pedido: o papel é conferido
    /// antes, e [`Campos::sem_repetidas`] depois).
    pub(crate) fn sem_conferir(v: Valor) -> R<Self> {
        match v {
            Valor::Objeto(p) => Ok(Self(p)),
            _ => Err(JsonInvalido),
        }
    }

    /// Recusa chave repetida. O objeto tem no máximo `MAX_CHAVES` chaves.
    pub(crate) fn sem_repetidas(&self) -> R<()> {
        for (i, (k, _)) in self.0.iter().enumerate() {
            if self.0[..i].iter().any(|(j, _)| j == k) {
                return Err(JsonInvalido);
            }
        }
        Ok(())
    }

    pub(crate) fn vazio() -> Self {
        Self(Vec::new())
    }

    /// Os pares que sobraram, na ordem do quadro.
    pub(crate) fn em_ordem(self) -> Vec<(String, Valor)> {
        self.0
    }

    pub(crate) fn tirar(&mut self, k: &str) -> Option<Valor> {
        let i = self.0.iter().position(|(c, _)| c == k)?;
        Some(self.0.remove(i).1)
    }

    /// Recusa campo que sobrou.
    pub(crate) fn fim(self) -> R<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(JsonInvalido)
        }
    }

    fn exigir(&mut self, k: &str) -> R<Valor> {
        self.tirar(k).ok_or(JsonInvalido)
    }

    pub(crate) fn texto(&mut self, k: &str, max: usize) -> R<String> {
        texto_de(self.exigir(k)?, max)
    }

    /// Texto opcional; ausente vira vazio (o "não informado" do [`crate::entry::Draft`]).
    pub(crate) fn texto_ou_vazio(&mut self, k: &str, max: usize) -> R<String> {
        match self.tirar(k) {
            Some(v) => texto_de(v, max),
            None => Ok(String::new()),
        }
    }

    pub(crate) fn texto_opc(&mut self, k: &str, max: usize) -> R<Option<String>> {
        self.tirar(k).map(|v| texto_de(v, max)).transpose()
    }

    pub(crate) fn segredo(&mut self, k: &str, max: usize) -> R<Secret> {
        segredo_de(self.exigir(k)?, max)
    }

    pub(crate) fn segredo_opc(&mut self, k: &str, max: usize) -> R<Option<Secret>> {
        self.tirar(k).map(|v| segredo_de(v, max)).transpose()
    }

    pub(crate) fn bytes(&mut self, k: &str, max: usize) -> R<Zeroizing<Vec<u8>>> {
        bytes_de(self.exigir(k)?, max)
    }

    pub(crate) fn inteiro(&mut self, k: &str) -> R<u64> {
        match self.exigir(k)? {
            Valor::Inteiro(n) => Ok(n),
            _ => Err(JsonInvalido),
        }
    }

    pub(crate) fn logico(&mut self, k: &str) -> R<bool> {
        match self.exigir(k)? {
            Valor::Logico(b) => Ok(b),
            _ => Err(JsonInvalido),
        }
    }

    /// Lista de até `max` itens.
    pub(crate) fn lista(&mut self, k: &str, max: usize) -> R<Vec<Valor>> {
        lista_de(self.exigir(k)?, max)
    }

    /// Lista opcional; ausente vira vazia.
    pub(crate) fn lista_ou_vazia(&mut self, k: &str, max: usize) -> R<Vec<Valor>> {
        match self.tirar(k) {
            Some(v) => lista_de(v, max),
            None => Ok(Vec::new()),
        }
    }

    pub(crate) fn objeto(&mut self, k: &str) -> R<Campos> {
        Campos::de(self.exigir(k)?)
    }

    /// Objeto opcional; ausente vira vazio.
    pub(crate) fn objeto_ou_vazio(&mut self, k: &str) -> R<Campos> {
        match self.tirar(k) {
            Some(v) => Campos::de(v),
            None => Ok(Campos::vazio()),
        }
    }
}

/// Texto comum (não secreto) de até `max` bytes. Sai do `Zeroizing` sem cópia.
pub(crate) fn texto_de(v: Valor, max: usize) -> R<String> {
    match v {
        Valor::Texto(mut t) if t.len() <= max => Ok(core::mem::take(&mut *t)),
        _ => Err(JsonInvalido),
    }
}

/// Bytes em base64, até `max` decodificados, num buffer que zera.
pub(crate) fn bytes_de(v: Valor, max: usize) -> R<Zeroizing<Vec<u8>>> {
    match v {
        Valor::Texto(t) => de_base64(&t, max),
        _ => Err(JsonInvalido),
    }
}

/// Segredo em base64 (todo segredo do protocolo vai assim; o texto em base64 zera ao
/// sair junto com o valor do quadro).
pub(crate) fn segredo_de(v: Valor, max: usize) -> R<Secret> {
    let mut b = bytes_de(v, max)?;
    Ok(Secret::from_bytes(core::mem::take(&mut *b)))
}

pub(crate) fn lista_de(v: Valor, max: usize) -> R<Vec<Valor>> {
    match v {
        Valor::Lista(l) if l.len() <= max => Ok(l),
        _ => Err(JsonInvalido),
    }
}

/// Lista de textos comuns.
pub(crate) fn textos_de(v: Vec<Valor>, max: usize) -> R<Vec<String>> {
    v.into_iter().map(|t| texto_de(t, max)).collect()
}
