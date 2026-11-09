//! JSON estrito do protocolo: leitor e escritor próprios, sem dependência nova (S14).
//!
//! O subconjunto aceito é o que o protocolo usa, e nada além:
//!
//! - o topo é sempre um objeto;
//! - texto em UTF-8 válido, sem caractere de controle cru, com os escapes do JSON
//!   (`\uXXXX` com par substituto; substituto solto é recusado);
//! - número só inteiro sem sinal, sem zero à esquerda, sem fração nem expoente, até
//!   `u64::MAX` (negativo é recusado);
//! - `true`, `false`, `null`, listas e objetos;
//! - chave repetida no mesmo objeto é recusada, mas por [`super::campos::Campos`], ao
//!   ler o objeto, para que no topo isso aconteça depois da conferência do papel (dois
//!   valores para o mesmo campo
//!   permitiriam ler um e conferir outro);
//! - profundidade até [`MAX_PROFUNDIDADE`], até [`MAX_NOS`] valores no quadro e até
//!   [`MAX_CHAVES`] chaves por objeto, para que um quadro pequeno não vire muita memória.
//!
//! Segredo (S8): todo texto lido fica num `Zeroizing<String>` com a capacidade exata
//! calculada antes (o texto decodificado nunca é maior que o cru), então nenhuma
//! realocação deixa cópia sem zerar. O escritor calcula o tamanho final numa passada que
//! só conta e escreve num `Zeroizing<Vec<u8>>` com essa capacidade.
//!
//! Erros não carregam posição nem trecho do quadro: o quadro pode ter segredo (S13).

use base64ct::{Base64, Encoding};
use zeroize::{Zeroize, Zeroizing};

/// Maior profundidade de aninhamento (o topo conta 1).
pub(crate) const MAX_PROFUNDIDADE: usize = 8;
/// Maior número de valores num pedido (cada texto, número, item e objeto conta 1).
pub(crate) const MAX_NOS: usize = 20_000;
/// Maior número de valores numa resposta: a lista de um cofre cheio (cada item tem
/// até 9 valores mais as tags e o resumo do app, D65 c).
pub(crate) const MAX_NOS_RESPOSTA: usize = crate::limites::MAX_ENTRADAS
    * (9 + crate::limites::MAX_TAGS + super::resposta::MAX_NOS_RESUMO)
    + 1024;
/// Maior número de chaves num objeto.
pub(crate) const MAX_CHAVES: usize = 32;

/// JSON recusado. Sem detalhe de propósito.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JsonInvalido;

/// Valor lido. O texto zera ao sair.
pub(crate) enum Valor {
    Nulo,
    Logico(bool),
    Inteiro(u64),
    Texto(Zeroizing<String>),
    Lista(Vec<Valor>),
    Objeto(Vec<(String, Valor)>),
}

impl core::fmt::Debug for Valor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Nunca o conteúdo: o texto pode ser segredo (S13).
        f.write_str(match self {
            Valor::Nulo => "Nulo",
            Valor::Logico(_) => "Logico",
            Valor::Inteiro(_) => "Inteiro",
            Valor::Texto(_) => "Texto(••••)",
            Valor::Lista(_) => "Lista",
            Valor::Objeto(_) => "Objeto",
        })
    }
}

/// Lê um quadro inteiro, com até `max_nos` valores. O topo tem de ser um objeto, e
/// depois dele só espaço.
pub(crate) fn ler(bytes: &[u8], max_nos: usize) -> Result<Valor, JsonInvalido> {
    let texto = core::str::from_utf8(bytes).map_err(|_| JsonInvalido)?;
    let mut l = Leitor {
        b: texto.as_bytes(),
        i: 0,
        nos: 0,
        max_nos,
    };
    l.espaco();
    if l.espiar() != Some(b'{') {
        return Err(JsonInvalido);
    }
    let v = l.valor(1)?;
    l.espaco();
    if l.i != l.b.len() {
        return Err(JsonInvalido);
    }
    Ok(v)
}

struct Leitor<'a> {
    /// UTF-8 já conferido.
    b: &'a [u8],
    i: usize,
    nos: usize,
    max_nos: usize,
}

impl Leitor<'_> {
    fn espiar(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn espaco(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.espiar() {
            self.i += 1;
        }
    }

    fn esperar(&mut self, c: u8) -> Result<(), JsonInvalido> {
        if self.espiar() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(JsonInvalido)
        }
    }

    fn literal(&mut self, palavra: &[u8]) -> Result<(), JsonInvalido> {
        if self.b[self.i..].starts_with(palavra) {
            self.i += palavra.len();
            Ok(())
        } else {
            Err(JsonInvalido)
        }
    }

    fn contar_no(&mut self) -> Result<(), JsonInvalido> {
        self.nos += 1;
        if self.nos > self.max_nos {
            Err(JsonInvalido)
        } else {
            Ok(())
        }
    }

    fn valor(&mut self, profundidade: usize) -> Result<Valor, JsonInvalido> {
        if profundidade > MAX_PROFUNDIDADE {
            return Err(JsonInvalido);
        }
        self.contar_no()?;
        match self.espiar().ok_or(JsonInvalido)? {
            b'{' => self.objeto(profundidade),
            b'[' => self.lista(profundidade),
            b'"' => Ok(Valor::Texto(self.texto()?)),
            b't' => self.literal(b"true").map(|()| Valor::Logico(true)),
            b'f' => self.literal(b"false").map(|()| Valor::Logico(false)),
            b'n' => self.literal(b"null").map(|()| Valor::Nulo),
            b'0'..=b'9' => self.inteiro().map(Valor::Inteiro),
            _ => Err(JsonInvalido),
        }
    }

    fn objeto(&mut self, profundidade: usize) -> Result<Valor, JsonInvalido> {
        self.esperar(b'{')?;
        let mut pares: Vec<(String, Valor)> = Vec::new();
        self.espaco();
        if self.espiar() == Some(b'}') {
            self.i += 1;
            return Ok(Valor::Objeto(pares));
        }
        loop {
            self.espaco();
            if self.espiar() != Some(b'"') {
                return Err(JsonInvalido);
            }
            let mut chave = self.texto()?;
            let chave = core::mem::take(&mut *chave);
            if pares.len() >= MAX_CHAVES {
                return Err(JsonInvalido);
            }
            self.espaco();
            self.esperar(b':')?;
            self.espaco();
            let v = self.valor(profundidade + 1)?;
            pares.push((chave, v));
            self.espaco();
            match self.espiar() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Valor::Objeto(pares));
                }
                _ => return Err(JsonInvalido),
            }
        }
    }

    fn lista(&mut self, profundidade: usize) -> Result<Valor, JsonInvalido> {
        self.esperar(b'[')?;
        let mut itens = Vec::new();
        self.espaco();
        if self.espiar() == Some(b']') {
            self.i += 1;
            return Ok(Valor::Lista(itens));
        }
        loop {
            self.espaco();
            itens.push(self.valor(profundidade + 1)?);
            self.espaco();
            match self.espiar() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Valor::Lista(itens));
                }
                _ => return Err(JsonInvalido),
            }
        }
    }

    fn inteiro(&mut self) -> Result<u64, JsonInvalido> {
        let inicio = self.i;
        let mut n: u64 = 0;
        while let Some(c @ b'0'..=b'9') = self.espiar() {
            n = n
                .checked_mul(10)
                .and_then(|n| n.checked_add(u64::from(c - b'0')))
                .ok_or(JsonInvalido)?;
            self.i += 1;
        }
        let digitos = self.i - inicio;
        // Zero à esquerda, fração e expoente ficam fora.
        if digitos > 1 && self.b[inicio] == b'0' {
            return Err(JsonInvalido);
        }
        if let Some(b'.' | b'e' | b'E' | b'-' | b'+') = self.espiar() {
            return Err(JsonInvalido);
        }
        Ok(n)
    }

    /// Lê um texto entre aspas. Primeiro acha o fim (o cru é um limite do decodificado),
    /// depois decodifica num buffer com essa capacidade, sem realocar.
    fn texto(&mut self) -> Result<Zeroizing<String>, JsonInvalido> {
        self.esperar(b'"')?;
        let inicio = self.i;
        let mut j = inicio;
        loop {
            match self.b.get(j).copied().ok_or(JsonInvalido)? {
                b'"' => break,
                b'\\' => j += 2,
                c if c < 0x20 => return Err(JsonInvalido),
                _ => j += 1,
            }
        }
        let fim = j;
        let mut saida = Zeroizing::new(String::with_capacity(fim - inicio));
        let capacidade = saida.capacity();
        let mut k = inicio;
        while k < fim {
            let c = self.b[k];
            if c != b'\\' {
                // Copia o trecho sem escape de uma vez (os limites caem em fronteira de
                // caractere: `\\` e `"` são ASCII).
                let mut e = k;
                while e < fim && self.b[e] != b'\\' {
                    e += 1;
                }
                let trecho = core::str::from_utf8(&self.b[k..e]).map_err(|_| JsonInvalido)?;
                saida.push_str(trecho);
                k = e;
                continue;
            }
            let esc = *self.b.get(k + 1).ok_or(JsonInvalido)?;
            k += 2;
            let ch = match esc {
                b'"' => '"',
                b'\\' => '\\',
                b'/' => '/',
                b'b' => '\u{8}',
                b'f' => '\u{c}',
                b'n' => '\n',
                b'r' => '\r',
                b't' => '\t',
                b'u' => {
                    let alto = hex4(self.b.get(k..k + 4).ok_or(JsonInvalido)?)?;
                    k += 4;
                    let cp = if (0xD800..0xDC00).contains(&alto) {
                        if self.b.get(k..k + 2) != Some(b"\\u") {
                            return Err(JsonInvalido);
                        }
                        let baixo = hex4(self.b.get(k + 2..k + 6).ok_or(JsonInvalido)?)?;
                        if !(0xDC00..0xE000).contains(&baixo) {
                            return Err(JsonInvalido);
                        }
                        k += 6;
                        0x10000 + ((alto - 0xD800) << 10) + (baixo - 0xDC00)
                    } else if (0xDC00..0xE000).contains(&alto) {
                        return Err(JsonInvalido);
                    } else {
                        alto
                    };
                    char::from_u32(cp).ok_or(JsonInvalido)?
                }
                _ => return Err(JsonInvalido),
            };
            saida.push(ch);
        }
        debug_assert_eq!(saida.capacity(), capacidade, "o texto realocou");
        self.i = fim + 1;
        Ok(saida)
    }
}

fn hex4(b: &[u8]) -> Result<u32, JsonInvalido> {
    let mut n = 0u32;
    for &c in b {
        let d = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => return Err(JsonInvalido),
        };
        n = n * 16 + u32::from(d);
    }
    Ok(n)
}

/// Decodifica base64 padrão (com `=`) num buffer que zera ao sair, com a capacidade
/// exata. Recusa acima de `maximo` bytes decodificados antes de alocar. O `base64ct`
/// (RustCrypto) é de tempo constante e recusa forma não canônica.
pub(crate) fn de_base64(texto: &str, maximo: usize) -> Result<Zeroizing<Vec<u8>>, JsonInvalido> {
    let n = texto.len();
    if !n.is_multiple_of(4) {
        return Err(JsonInvalido);
    }
    let limite = n / 4 * 3;
    if limite > maximo.saturating_add(2) {
        return Err(JsonInvalido);
    }
    let mut buf = Zeroizing::new(vec![0u8; limite]);
    let tamanho = Base64::decode(texto.as_bytes(), &mut buf)
        .map_err(|_| JsonInvalido)?
        .len();
    if tamanho > maximo {
        return Err(JsonInvalido);
    }
    // `truncate` não realoca; os bytes que sobram zeram junto com o buffer.
    buf[tamanho..].zeroize();
    buf.truncate(tamanho);
    Ok(buf)
}

/// Tamanho do base64 padrão de `n` bytes.
pub(crate) const fn tamanho_base64(n: usize) -> usize {
    n.div_ceil(3) * 4
}

// ---- Escritor ----

/// Escreve JSON em duas passadas: [`codificar`] chama a função uma vez contando e outra
/// escrevendo, num buffer com a capacidade exata.
pub(crate) struct Escritor {
    /// `None` na passada que só conta.
    buf: Option<Zeroizing<Vec<u8>>>,
    tamanho: usize,
    /// Por nível aberto: ainda não escreveu nenhum item?
    primeiro: Vec<bool>,
}

/// O resultado passaria do limite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GrandeDemais;

/// Codifica com `escrever`, recusando acima de `limite` bytes antes de alocar.
pub(crate) fn codificar(
    limite: usize,
    escrever: impl Fn(&mut Escritor),
) -> Result<Zeroizing<Vec<u8>>, GrandeDemais> {
    let mut contador = Escritor {
        buf: None,
        tamanho: 0,
        primeiro: Vec::new(),
    };
    escrever(&mut contador);
    let total = contador.tamanho;
    if total > limite {
        return Err(GrandeDemais);
    }
    let mut e = Escritor {
        buf: Some(Zeroizing::new(Vec::with_capacity(total))),
        tamanho: 0,
        primeiro: Vec::new(),
    };
    escrever(&mut e);
    let buf = e.buf.take().unwrap_or_default();
    // As duas passadas são a mesma função; se divergirem, é erro de programação.
    assert_eq!(buf.len(), total, "as passadas do escritor divergiram");
    assert_eq!(buf.capacity(), total, "o escritor realocou");
    Ok(buf)
}

impl Escritor {
    fn bruto(&mut self, b: &[u8]) {
        self.tamanho += b.len();
        if let Some(buf) = self.buf.as_mut() {
            // Nunca cresce: a capacidade foi calculada pela passada que conta.
            if buf.capacity() - buf.len() >= b.len() {
                buf.extend_from_slice(b);
            }
        }
    }

    fn separador(&mut self) {
        if let Some(p) = self.primeiro.last_mut() {
            if *p {
                *p = false;
            } else {
                self.bruto(b",");
            }
        }
    }

    pub(crate) fn abre_objeto(&mut self) {
        self.bruto(b"{");
        self.primeiro.push(true);
    }

    pub(crate) fn fecha_objeto(&mut self) {
        self.primeiro.pop();
        self.bruto(b"}");
    }

    pub(crate) fn abre_lista(&mut self) {
        self.bruto(b"[");
        self.primeiro.push(true);
    }

    pub(crate) fn fecha_lista(&mut self) {
        self.primeiro.pop();
        self.bruto(b"]");
    }

    /// Chave de objeto (as chaves são fixas do protocolo).
    pub(crate) fn chave(&mut self, k: &str) {
        self.separador();
        self.texto_cru(k);
        self.bruto(b":");
        // O valor a seguir não leva vírgula.
        self.primeiro.push(true);
    }

    /// Antes de cada item de lista.
    pub(crate) fn item(&mut self) {
        self.separador();
        self.primeiro.push(true);
    }

    /// Depois do valor de uma chave ou de um item.
    fn fim_valor(&mut self) {
        // Os valores escalares e compostos abaixo fecham o nível de `chave`/`item`.
        self.primeiro.pop();
    }

    pub(crate) fn texto(&mut self, s: &str) {
        self.texto_cru(s);
        self.fim_valor();
    }

    pub(crate) fn inteiro(&mut self, n: u64) {
        let mut tmp = [0u8; 20];
        let mut i = tmp.len();
        let mut v = n;
        loop {
            i -= 1;
            tmp[i] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        self.bruto(&tmp[i..]);
        self.fim_valor();
    }

    pub(crate) fn logico(&mut self, b: bool) {
        self.bruto(if b { b"true" } else { b"false" });
        self.fim_valor();
    }

    /// Bytes em base64 padrão. O pedaço na pilha zera no fim.
    pub(crate) fn base64(&mut self, dados: &[u8]) {
        self.bruto(b"\"");
        let n = tamanho_base64(dados.len());
        self.tamanho += n;
        if let Some(buf) = self.buf.as_mut()
            && buf.capacity() - buf.len() >= n
        {
            let mut tmp = Zeroizing::new([0u8; 1024]);
            for pedaco in dados.chunks(768) {
                if let Ok(s) = Base64::encode(pedaco, &mut tmp[..]) {
                    buf.extend_from_slice(s.as_bytes());
                }
            }
        }
        self.bruto(b"\"");
        self.fim_valor();
    }

    /// Abre um objeto como valor de chave ou item.
    pub(crate) fn objeto(&mut self, f: impl FnOnce(&mut Escritor)) {
        self.abre_objeto();
        f(self);
        self.fecha_objeto();
        self.fim_valor();
    }

    /// Abre uma lista como valor de chave ou item.
    pub(crate) fn lista(&mut self, f: impl FnOnce(&mut Escritor)) {
        self.abre_lista();
        f(self);
        self.fecha_lista();
        self.fim_valor();
    }

    fn texto_cru(&mut self, s: &str) {
        self.bruto(b"\"");
        let b = s.as_bytes();
        let mut inicio = 0;
        for (i, &c) in b.iter().enumerate() {
            let esc: Option<&[u8]> = match c {
                b'"' => Some(b"\\\""),
                b'\\' => Some(b"\\\\"),
                b'\n' => Some(b"\\n"),
                b'\r' => Some(b"\\r"),
                b'\t' => Some(b"\\t"),
                c if c < 0x20 => None,
                _ => continue,
            };
            self.bruto(&b[inicio..i]);
            match esc {
                Some(e) => self.bruto(e),
                None => {
                    const HEX: &[u8; 16] = b"0123456789abcdef";
                    self.bruto(&[
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX[usize::from(c >> 4)],
                        HEX[usize::from(c & 0xf)],
                    ]);
                }
            }
            inicio = i + 1;
        }
        self.bruto(&b[inicio..]);
        self.bruto(b"\"");
    }
}
