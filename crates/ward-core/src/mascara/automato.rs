//! Autômato de Aho-Corasick dos padrões da máscara.
//!
//! Feito aqui, sem dependência, por dois motivos: (1) o autômato guarda os bytes dos
//! segredos, e uma biblioteca de busca guardaria cópias que não zeram ao sair (S8); (2)
//! a máscara precisa do estado entre pedaços da saída (o prefixo de segredo que ficou
//! no fim do pedaço) e da profundidade do estado, para segurar só o necessário.
//! Não é criptografia: é busca de texto, com tempo linear no tamanho da saída mesmo
//! com padrões repetitivos (`aaaa…` contra `aaaa…`), então a saída de um comando não
//! consegue travar o daemon.
//!
//! Todas as tabelas ficam em `Zeroizing<Vec<_>>` com a capacidade calculada antes de
//! inserir (nenhuma realocação deixa cópia em memória liberada) e zeram ao sair.

use zeroize::Zeroizing;

/// "Sem nó": a raiz (0) nunca é filha nem irmã de ninguém.
const NENHUM: u32 = 0;
const RAIZ: u32 = 0;

pub(super) struct Automato {
    /// Byte da aresta que chega a cada nó.
    byte: Zeroizing<Vec<u8>>,
    /// Primeiro filho (lista ligada pelos irmãos).
    filho: Zeroizing<Vec<u32>>,
    /// Próximo irmão.
    irmao: Zeroizing<Vec<u32>>,
    /// Ligação de falha.
    falha: Zeroizing<Vec<u32>>,
    /// Tamanho do maior padrão que termina neste estado (0 = nenhum).
    saida: Zeroizing<Vec<u32>>,
    /// Profundidade do nó = tamanho do maior prefixo de padrão que casa agora.
    prof: Zeroizing<Vec<u32>>,
    /// Filhos da raiz em tabela direta (a raiz é o estado mais visitado).
    raiz: Zeroizing<Vec<u32>>,
}

/// Garante espaço para mais `adicional` elementos sem deixar a alocação antiga em
/// claro: se precisar crescer, copia para uma nova e a antiga zera ao cair.
pub(super) fn reservar<T>(v: &mut Zeroizing<Vec<T>>, adicional: usize)
where
    T: zeroize::Zeroize + Clone,
{
    let precisa = v
        .len()
        .checked_add(adicional)
        .expect("máscara: tamanho fora do limite");
    if precisa <= v.capacity() {
        return;
    }
    let capacidade = precisa.max(v.capacity().saturating_mul(2)).max(16);
    let mut novo = Zeroizing::new(Vec::with_capacity(capacidade));
    novo.extend_from_slice(v);
    *v = novo;
}

fn empurrar<T>(v: &mut Zeroizing<Vec<T>>, x: T)
where
    T: zeroize::Zeroize + Clone,
{
    reservar(v, 1);
    v.push(x);
}

fn indice(n: u32) -> usize {
    n as usize
}

impl Automato {
    /// Autômato vazio com espaço para `nos` nós (incluindo a raiz).
    pub(super) fn com_capacidade(nos: usize) -> Self {
        let nos = nos.max(1);
        let mut a = Automato {
            byte: Zeroizing::new(Vec::with_capacity(nos)),
            filho: Zeroizing::new(Vec::with_capacity(nos)),
            irmao: Zeroizing::new(Vec::with_capacity(nos)),
            falha: Zeroizing::new(Vec::with_capacity(nos)),
            saida: Zeroizing::new(Vec::with_capacity(nos)),
            prof: Zeroizing::new(Vec::with_capacity(nos)),
            raiz: Zeroizing::new(vec![NENHUM; 256]),
        };
        a.novo_no(0, 0);
        a
    }

    fn novo_no(&mut self, byte: u8, prof: u32) -> u32 {
        let id = u32::try_from(self.byte.len()).expect("máscara: nós demais");
        empurrar(&mut self.byte, byte);
        empurrar(&mut self.filho, NENHUM);
        empurrar(&mut self.irmao, NENHUM);
        empurrar(&mut self.falha, RAIZ);
        empurrar(&mut self.saida, 0);
        empurrar(&mut self.prof, prof);
        id
    }

    /// Número de nós (para os testes de tamanho).
    #[cfg(test)]
    pub(super) fn nos(&self) -> usize {
        self.byte.len()
    }

    fn filho_de(&self, no: u32, b: u8) -> Option<u32> {
        if no == RAIZ {
            let c = self.raiz[usize::from(b)];
            return (c != NENHUM).then_some(c);
        }
        let mut c = self.filho[indice(no)];
        while c != NENHUM {
            if self.byte[indice(c)] == b {
                return Some(c);
            }
            c = self.irmao[indice(c)];
        }
        None
    }

    /// Acrescenta um padrão. Padrão vazio é ignorado (casaria com tudo).
    pub(super) fn inserir(&mut self, padrao: &[u8]) {
        if padrao.is_empty() {
            return;
        }
        let mut no = RAIZ;
        for &b in padrao {
            no = match self.filho_de(no, b) {
                Some(c) => c,
                None => {
                    let prof = self.prof[indice(no)] + 1;
                    let novo = self.novo_no(b, prof);
                    if no == RAIZ {
                        self.raiz[usize::from(b)] = novo;
                    } else {
                        self.irmao[indice(novo)] = self.filho[indice(no)];
                        self.filho[indice(no)] = novo;
                    }
                    novo
                }
            };
        }
        let i = indice(no);
        self.saida[i] = self.prof[i];
    }

    /// Calcula as ligações de falha (busca em largura) e propaga as saídas.
    pub(super) fn finalizar(&mut self) {
        let mut fila: Zeroizing<Vec<u32>> = Zeroizing::new(Vec::with_capacity(self.byte.len()));
        for b in 0..=255u8 {
            let c = self.raiz[usize::from(b)];
            if c != NENHUM {
                self.falha[indice(c)] = RAIZ;
                empurrar(&mut fila, c);
            }
        }
        let mut cabeca = 0;
        while cabeca < fila.len() {
            let u = fila[cabeca];
            cabeca += 1;
            let mut v = self.filho[indice(u)];
            while v != NENHUM {
                let b = self.byte[indice(v)];
                let mut f = self.falha[indice(u)];
                let alvo = loop {
                    if let Some(w) = self.filho_de(f, b) {
                        break w;
                    }
                    if f == RAIZ {
                        break RAIZ;
                    }
                    f = self.falha[indice(f)];
                };
                let iv = indice(v);
                self.falha[iv] = alvo;
                let herdada = self.saida[indice(alvo)];
                if herdada > self.saida[iv] {
                    self.saida[iv] = herdada;
                }
                empurrar(&mut fila, v);
                v = self.irmao[iv];
            }
        }
    }

    /// Próximo estado depois de ler `b`.
    pub(super) fn passo(&self, mut estado: u32, b: u8) -> u32 {
        loop {
            if let Some(c) = self.filho_de(estado, b) {
                return c;
            }
            if estado == RAIZ {
                return RAIZ;
            }
            estado = self.falha[indice(estado)];
        }
    }

    /// Tamanho do maior padrão que termina no estado (0 = nenhum).
    pub(super) fn saida(&self, estado: u32) -> usize {
        indice(self.saida[indice(estado)])
    }

    /// Quantos bytes do fim da entrada ainda podem ser começo de um padrão.
    pub(super) fn profundidade(&self, estado: u32) -> usize {
        indice(self.prof[indice(estado)])
    }
}
