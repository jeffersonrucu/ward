//! Máscara de saída (D11, S4): troca por `••••` todo valor secreto que aparecer na
//! saída do comando (stdout e stderr, cada um no seu [`Fluxo`]), inclusive quando o
//! valor chega inteiro num pedaço da saída (D34 revisto: nada é segurado; ver "Fluxo").
//!
//! O que entra na máscara, para cada segredo da entrada (campos secretos do tipo,
//! extras secretos, passphrase, semente TOTP) e para cada variável que a
//! [`Delivery`] marcou como segredo (`Variable::is_secret`, que inclui o
//! `DATABASE_URL` inteiro):
//! - o valor literal;
//! - percent-encoding em qualquer forma (maiúsculas, minúsculas, `+` para espaço,
//!   codificação parcial como a do `urllib.parse.quote`): não vira padrão, a saída é lida
//!   também decodificada (ver "Visões");
//! - base64 padrão e URL-safe, com e sem `=`, e o miolo do base64 do segredo nos três
//!   alinhamentos (pega o segredo dentro de um base64 maior, como `x:token`);
//! - base64 de `usuário:senha` (Basic auth) com o usuário da entrada;
//! - as **formas de texto** (D37, D54), cada uma só quando difere do valor literal: o
//!   escape JSON padrão (`\"`, `\\`, controles), com e sem `\/`; o JSON do Go (`&`,
//!   `<`, `>`, U+2028 e U+2029 como `\u0026`, `\u003c`, `\u003e`, `\u2028`,
//!   `\u2029`); o JSON com `ensure_ascii` do Python (todo caractere fora do ASCII como
//!   `\uXXXX` em minúsculas, com par substituto acima de U+FFFF); o JSON do Go com
//!   `ensure_ascii` (D58: os dois juntos, só quando o valor tem `&`, `<` ou `>` e
//!   caractere fora do ASCII); o `json_encode` do PHP
//!   (`\/` e `\uXXXX` juntos, só quando o valor tem `/` e caractere fora do ASCII); as
//!   aspas duplas do shell (`declare -p`, `export -p`: `\` antes de `$`, `` ` ``, `"` e
//!   `\`); as aspas simples do shell (`${X@Q}`, `set`, `set -x`: `'` vira `'\''`, só
//!   quando o valor tem `'`); e o `printf %q` do bash (barra antes de cada caractere
//!   especial, só para valor em ASCII visível e espaço; com controle ou fora do ASCII o
//!   bash troca para `$'…'`, que fica em RA3).
//!
//! Anexos: o conteúdo inteiro (e sem a quebra de linha final) e seu base64, e a passada
//! por linha. Todo segredo de campo, extra ou entrega com `\n` (a chave privada do
//! `servidor` e do `ftp/sftp`, um JSON de conta de serviço num extra, um token de `api`
//! de várias linhas) também passa por linha (D36, D54), além de inteiro. A passada por
//! linha cobre: cada linha com 6 caracteres ou mais, sem o `\r` do fim e também
//! aparada, com as formas de texto dela (sem base64 nem miolos, D54); os campos de 6+
//! de uma linha com tab (`cut -f`, TSV); e o valor de linhas `CHAVE=valor` (com ou sem
//! `export`), `"chave": "valor"` (JSON, chave com qualquer caractere menos `"`, também
//! colado `"k":"v"`) e `chave: valor` (YAML, também `chave : valor` e `chave:\tvalor`),
//! com e sem aspas, com 6 caracteres ou mais (D34). O valor de linha e o campo de tab
//! têm todas as variações de um segredo inteiro.
//!
//! Reauditoria da 19e (D61), na mesma passada por linha:
//! - **JSON na linha:** em toda linha de texto, e no segredo de uma linha só que, aparado,
//!   começa por `{` ou `[`, cada par `"chave"` + `:` + string JSON, em qualquer posição
//!   da linha (objeto minificado, aninhado, duas chaves na linha), dá a string
//!   decodificada pelos escapes do JSON como valor de linha (6+; escape inválido descarta
//!   o par). Elemento de array sem chave e o flow do YAML ficam em RA3;
//! - **comentário no fim** (YAML e dotenv): valor sem aspas até antes do primeiro espaço
//!   ou tab seguido de `#`; valor entre aspas seguido de espaços e `# …` dá o miolo;
//! - **chave YAML composta** (`private key:`, `aws/secret:`): a chave vai até o primeiro
//!   `:` seguido de espaço, tab ou fim, sem `#` nem aspas, e não começa por `-`, `{`, `[`;
//! - **`''` do YAML:** no valor entre aspas simples, `''` vira `'`;
//! - **campos de tab de linha que não é texto:** cada campo de 6+ bytes só pelo literal
//!   (aparado e cru) e pelo percent, sem formas de texto nem base64.
//!
//! Cada candidato a mais só entra quando difere do que a linha já gera.
//!
//! Orçamento (D29): [`custo_da_entrada`] conta os padrões de uma entrada e da entrega
//! dela pelo mesmo [`Gerador`]; a validação recusa a entrada acima de
//! [`MASK_BUDGET`].
//!
//! O código TOTP não é mascarado (o agente o recebe, R8): `WARD_TOTP` não é segredo na
//! entrega. Segredo curto (menos de 6 caracteres) é mascarado do mesmo jeito;
//! [`Mascara::tem_segredo_curto`] avisa o app.
//!
//! # Visões
//! Cada [`Fluxo`] passa os bytes por três leituras da mesma saída no mesmo autômato:
//! crua, com `%XX` decodificado e com `%XX` e `+` decodificados. Um casamento numa
//! visão decodificada é levado de volta aos bytes originais. Assim nenhuma combinação
//! de percent-encoding escapa, sem multiplicar padrões.
//!
//! # Fluxo
//! O fluxo **nunca segura nem atrasa** saída (D34 revisto, auditoria do M4): cada
//! pedaço lido do comando sai na hora, inteiro, com `••••` só no lugar do que completou
//! um padrão (o segredo inteiro, ou uma variação inteira) até o fim daquele pedaço.
//! Segurar o fim que podia ser começo de segredo virava oráculo: soltar o prefixo como
//! marca (no pty) ou só depois de mais saída (sem pty) dizia ao comando, por tempo ou
//! pelo que aparecia, se um prefixo era o começo da senha, e o agente recuperou a senha
//! caractere a caractere. Agora a saída de um pedaço depende só dos bytes dele e dos
//! anteriores, nunca de quanto deles casa com um segredo pela metade.
//!
//! O estado do autômato continua de um pedaço para o outro: um segredo que chega
//! quebrado entre dois pedaços tem o pedaço do fim mascarado (o começo, que já saiu,
//! fica como saiu). A garantia de S4 é o segredo inteiro dentro de um mesmo pedaço
//! (D34: segredo impresso em pedaços não é cenário de uso). Isso não abre oráculo novo:
//! a marca só aparece quando a saída já contém o padrão inteiro.
//!
//! Trechos cobertos que se sobrepõem ou se encostam viram um `••••` só. O que não é
//! segredo sai byte a byte igual (UTF-8 inválido e binário inclusive). O pedaço em
//! trabalho fica em memória que zera.
//!
//! Fora do modelo de ameaça (escopo › "Segurança"): um processo feito para driblar a
//! máscara (inverter a senha, imprimir metade no stdout e metade no stderr). Quem barra
//! é a aprovação.

mod automato;

use core::fmt;
use std::collections::VecDeque;

use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::delivery::{self, Delivery, DeliveryError, ResourceError, Resources};
use crate::entry::{Data, Entry, Secret, SecretOrigin};
use automato::{Automato, reservar};

#[cfg(test)]
mod testes;

/// O que substitui cada trecho secreto.
pub const MARCA: &str = "••••";

// Os limites estão em `crate::limits` (D32); ficam aqui também pelo nome antigo.
pub use crate::limits::{MASK_BUDGET, PATTERN_LIMIT, TRUSTED_MINIMUM};

/// Por que a máscara não foi montada. Só texto fixo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroMascara {
    /// Os segredos (em geral, anexos grandes) passam de [`PATTERN_LIMIT`] ao montar,
    /// ou de [`MASK_BUDGET`] no [`custo_da_entrada`].
    GrandeDemais,
    /// [`custo_da_entrada`]: a entrega da entrada não pôde ser montada (defesa em
    /// profundidade; a validação deveria ter barrado antes).
    SemEntrega,
}

impl fmt::Display for ErroMascara {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroMascara::GrandeDemais => {
                f.write_str("máscara não montada: os segredos da entrada são grandes demais")
            }
            ErroMascara::SemEntrega => {
                f.write_str("máscara não montada: a entrega da entrada falhou")
            }
        }
    }
}

impl std::error::Error for ErroMascara {}

/// Junta as entradas (a principal e a do salto ou do túnel) e as entregas cujos
/// segredos a máscara cobre.
#[derive(Default)]
pub struct ConstrutorMascara<'a> {
    entradas: Vec<&'a Entry>,
    entregas: Vec<&'a Delivery>,
}

impl fmt::Debug for ConstrutorMascara<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ConstrutorMascara(••••)")
    }
}

impl<'a> ConstrutorMascara<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cobre os segredos da entrada (V2.13) e o Basic auth com o usuário dela.
    pub fn entrada(mut self, entrada: &'a Entry) -> Self {
        self.entradas.push(entrada);
        self
    }

    /// Cobre os valores que a entrega marcou como segredo (`DATABASE_URL`, senha…).
    pub fn entrega(mut self, entrega: &'a Delivery) -> Self {
        self.entregas.push(entrega);
        self
    }

    pub fn construir(self) -> Result<Mascara, ErroMascara> {
        // Primeira passada: só soma, para alocar o autômato uma vez no tamanho certo.
        let total = somar(&self, PATTERN_LIMIT)?;
        let mut automato = Automato::com_capacidade(total + 1);
        visitar(&self, &mut |p| {
            automato.inserir(p);
            Ok(())
        })?;
        automato.finalizar();
        let curto = self.entradas.iter().any(|e| {
            e.secrets().iter().any(|s| {
                !matches!(s.origin, SecretOrigin::Attachment(_))
                    && caracteres(s.value.expose()) < TRUSTED_MINIMUM
            })
        });
        Ok(Mascara { automato, curto })
    }
}

/// A máscara montada. Sem `Clone`; zera ao sair; `Debug` não mostra nada.
pub struct Mascara {
    automato: Automato,
    curto: bool,
}

// Todas as tabelas do autômato são `Zeroizing`.
impl ZeroizeOnDrop for Mascara {}

impl fmt::Debug for Mascara {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Mascara(••••)")
    }
}

impl Mascara {
    /// Atalho: a máscara de uma entrada e, se houver, da entrega dela.
    pub fn da_entrada(entrada: &Entry, entrega: Option<&Delivery>) -> Result<Self, ErroMascara> {
        let mut c = ConstrutorMascara::new().entrada(entrada);
        if let Some(g) = entrega {
            c = c.entrega(g);
        }
        c.construir()
    }

    /// Algum segredo (fora os anexos) tem menos de 6 caracteres: o app avisa que a
    /// máscara fica pouco confiável (D11).
    pub fn tem_segredo_curto(&self) -> bool {
        self.curto
    }

    /// Um fluxo novo. Um para o stdout e outro para o stderr (D11: em separado).
    pub fn fluxo(&self) -> Fluxo<'_> {
        Fluxo {
            mascara: self,
            pendente: Zeroizing::new(Vec::new()),
            emitido: 0,
            lido: 0,
            visoes: [
                Visao::new(Modo::Cru),
                Visao::new(Modo::Percent),
                Visao::new(Modo::Formulario),
            ],
            cobertos: Vec::new(),
            em_marca: false,
        }
    }

    /// Mascara uma saída inteira de uma vez (equivale a um fluxo com um pedaço só).
    pub fn mascarar_tudo(&self, saida_do_comando: &[u8]) -> Vec<u8> {
        let mut saida = Vec::with_capacity(saida_do_comando.len());
        let mut f = self.fluxo();
        f.mascarar(saida_do_comando, &mut saida);
        f.terminar(&mut saida);
        saida
    }
}

// ---------------------------------------------------------------------------------
// Orçamento (D29)
// ---------------------------------------------------------------------------------

/// Soma dos padrões que [`ConstrutorMascara::construir`] geraria, pela mesma função
/// ([`visitar`] e o [`Gerador`]). Para assim que passa de `limite`.
pub(crate) fn somar(c: &ConstrutorMascara<'_>, limite: usize) -> Result<usize, ErroMascara> {
    let mut total: usize = 0;
    visitar(c, &mut |p| {
        total = total
            .checked_add(p.len())
            .filter(|t| *t <= limite)
            .ok_or(ErroMascara::GrandeDemais)?;
        Ok(())
    })?;
    Ok(total)
}

/// Recursos de mentira para medir o custo da entrega sem tocar no sistema. Os valores
/// que eles dão não são segredo (D11: o código TOTP e o caminho não entram na máscara);
/// a porta do túnel é a mais longa, para o `DATABASE_URL` medido nunca ser menor que o
/// do `ward run` (com túnel o host é sempre `127.0.0.1`).
struct RecursosDoOrcamento;

impl Resources for RecursosDoOrcamento {
    fn totp_code(&mut self, _semente: &Secret) -> Result<Zeroizing<String>, ResourceError> {
        Ok(Zeroizing::new(String::from("000000")))
    }

    fn file(&mut self, _nome: &str, _conteudo: &Secret) -> Result<String, ResourceError> {
        Ok(String::from("/orcamento"))
    }

    fn tunnel(&mut self, _servidor: &str, _host: &str, _porta: u16) -> Result<u16, ResourceError> {
        Ok(u16::MAX)
    }
}

/// Custo da entrada na máscara (D29): a soma dos padrões da entrada e da entrega dela,
/// contada pelo mesmo [`Gerador`] que monta a máscara do `ward run`. Recusa com
/// [`ErroMascara::GrandeDemais`] acima de [`MASK_BUDGET`]. A `nota segura` não
/// tem entrega (D10), então fica fora do orçamento e custa 0.
///
/// Como cada entrada cabe em metade de [`PATTERN_LIMIT`], as duas de um uso combinado
/// (D19) cabem juntas: nenhuma entrada aceita pelo cofre fica sem `ward run`.
pub fn custo_da_entrada(entrada: &Entry) -> Result<usize, ErroMascara> {
    let entrega = match delivery::build(entrada, &mut RecursosDoOrcamento) {
        Ok(g) => g,
        Err(DeliveryError::TypeWithoutDelivery) => return Ok(0),
        Err(_) => return Err(ErroMascara::SemEntrega),
    };
    let c = ConstrutorMascara::new().entrada(entrada).entrega(&entrega);
    somar(&c, MASK_BUDGET)
}

// ---------------------------------------------------------------------------------
// Padrões
// ---------------------------------------------------------------------------------

type Visitante<'f> = dyn FnMut(&[u8]) -> Result<(), ErroMascara> + 'f;

fn visitar(c: &ConstrutorMascara<'_>, f: &mut Visitante<'_>) -> Result<(), ErroMascara> {
    let mut g = Gerador::novo();
    for e in &c.entradas {
        let usuario = usuario(e.data());
        for s in e.secrets() {
            let valor = s.value.expose();
            match s.origin {
                SecretOrigin::Attachment(_) => g.anexo(valor, f)?,
                // D36: segredo de várias linhas também por linha, como os anexos.
                SecretOrigin::Extra(_) => g.segredo_de_campo(valor, f)?,
                SecretOrigin::Field(_) => {
                    g.segredo_de_campo(valor, f)?;
                    if let Some(u) = usuario {
                        g.basic(u.as_bytes(), valor, f)?;
                    }
                }
            }
        }
    }
    for g_ in &c.entregas {
        for v in g_.variables().iter().filter(|v| v.is_secret()) {
            // D54: valor secreto de entrega com `\n` (o token de `api` de várias
            // linhas) também por linha, mesmo sem `.entrada(…)` no construtor; e o JSON
            // de uma linha só (D61), do mesmo jeito.
            g.segredo_de_campo(v.expose().as_bytes(), f)?;
        }
    }
    Ok(())
}

fn usuario(d: &Data) -> Option<&str> {
    match d {
        Data::Login(l) => Some(&l.username),
        Data::Database(b) => Some(&b.username),
        Data::Server(s) => Some(&s.username),
        Data::Smtp(s) => Some(&s.username),
        Data::FtpSftp(s) => Some(&s.username),
        Data::Api(_) | Data::Aws(_) | Data::SecureNote(_) | Data::Generic => None,
    }
}

const B64_PADRAO: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const B64_URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Base64 (RFC 4648) de `dados`, acrescentado a `saida`. Não é cripto: só codificação.
fn base64(dados: &[u8], alfabeto: &[u8; 64], com_igual: bool, saida: &mut Zeroizing<Vec<u8>>) {
    reservar(saida, dados.len().div_ceil(3) * 4);
    for bloco in dados.chunks(3) {
        let b0 = bloco[0];
        let b1 = bloco.get(1).copied().unwrap_or(0);
        let b2 = bloco.get(2).copied().unwrap_or(0);
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        let chars = bloco.len() + 1;
        for i in 0..4 {
            if i < chars {
                saida.push(alfabeto[((n >> (18 - 6 * i)) & 63) as usize]);
            } else if com_igual {
                saida.push(b'=');
            }
        }
    }
}

/// Monta os padrões de cada segredo em buffers que zeram (e que crescem sem deixar
/// cópia: [`reservar`]).
struct Gerador {
    tmp: Zeroizing<Vec<u8>>,
    tmp2: Zeroizing<Vec<u8>>,
    /// Valor decodificado (string JSON, `''` do YAML) que vira segredo inteiro. Sai do
    /// `Gerador` enquanto é usado ([`core::mem::take`]) e volta zerado.
    tmp3: Zeroizing<Vec<u8>>,
}

impl Gerador {
    fn novo() -> Self {
        Gerador {
            tmp: Zeroizing::new(Vec::new()),
            tmp2: Zeroizing::new(Vec::new()),
            tmp3: Zeroizing::new(Vec::new()),
        }
    }

    /// Segredo de campo, extra ou entrega: inteiro; por linha se tem `\n` (D36, D54);
    /// e, se é uma linha só de texto que aparada começa por `{` ou `[`, os valores JSON
    /// dela (D61).
    fn segredo_de_campo(&mut self, s: &[u8], f: &mut Visitante<'_>) -> Result<(), ErroMascara> {
        self.segredo(s, f)?;
        if s.contains(&b'\n') {
            self.linhas(s, f)?;
        } else if matches!(s.trim_ascii().first(), Some(b'{' | b'['))
            && core::str::from_utf8(s).is_ok()
        {
            self.json_da_linha(s, None, f)?;
        }
        Ok(())
    }

    /// Valor de linha (D34): segredo inteiro, se tem o mínimo.
    fn valor_minimo(&mut self, v: &[u8], f: &mut Visitante<'_>) -> Result<(), ErroMascara> {
        if caracteres(v) >= TRUSTED_MINIMUM {
            self.segredo(v, f)?;
        }
        Ok(())
    }

    /// Os pares `"chave": "string"` da linha de texto (D61), em qualquer posição: cada
    /// string decodificada com 6+ caracteres é valor de linha. `ja` é o valor que a linha
    /// já gerou pelo parser de `chave: valor` (não se repete).
    fn json_da_linha(
        &mut self,
        linha: &[u8],
        ja: Option<&[u8]>,
        f: &mut Visitante<'_>,
    ) -> Result<(), ErroMascara> {
        let mut buf = core::mem::take(&mut self.tmp3);
        let mut r = Ok(());
        let mut i = 0usize;
        while let Some(p) = linha[i..].iter().position(|b| *b == b'"') {
            let Some(fim_chave) = fim_string_json(linha, i + p) else {
                break;
            };
            let j = pular_branco_json(linha, fim_chave);
            if linha.get(j) != Some(&b':') {
                i = fim_chave;
                continue;
            }
            let j = pular_branco_json(linha, j + 1);
            if linha.get(j) != Some(&b'"') {
                i = fim_chave;
                continue;
            }
            let Some(fim_valor) = fim_string_json(linha, j) else {
                break;
            };
            buf.clear();
            if decodificar_string_json(&linha[j + 1..fim_valor - 1], &mut buf)
                && ja != Some(&buf[..])
            {
                r = self.valor_minimo(&buf, f);
                if r.is_err() {
                    break;
                }
            }
            i = fim_valor;
        }
        buf.zeroize();
        self.tmp3 = buf;
        r
    }

    /// Os candidatos a mais de um valor de `CHAVE=valor` ou `chave: valor` (D61):
    /// comentário no fim e, no YAML (`yaml`), o `''` das aspas simples.
    fn comentario_e_aspas(
        &mut self,
        valor: &[u8],
        yaml: bool,
        f: &mut Visitante<'_>,
    ) -> Result<(), ErroMascara> {
        let Some(&q) = valor.first() else {
            return Ok(());
        };
        if !matches!(q, b'"' | b'\'') {
            if let Some(c) = antes_de_comentario(valor) {
                self.valor_minimo(c, f)?;
            }
            return Ok(());
        }
        let base = sem_aspas(valor);
        let miolo = miolo_antes_de_comentario(valor);
        if let Some(m) = miolo
            && Some(m) != base
        {
            self.valor_minimo(m, f)?;
        }
        if yaml
            && q == b'\''
            && let Some(dentro) = miolo.or(base)
            && dentro.windows(2).any(|w| w == b"''")
        {
            let mut buf = core::mem::take(&mut self.tmp3);
            buf.clear();
            reservar(&mut buf, dentro.len());
            let mut k = 0;
            while k < dentro.len() {
                buf.push(dentro[k]);
                k += if dentro[k] == b'\'' && dentro.get(k + 1) == Some(&b'\'') {
                    2
                } else {
                    1
                };
            }
            let r = self.valor_minimo(&buf, f);
            buf.zeroize();
            self.tmp3 = buf;
            r?;
        }
        Ok(())
    }

    fn segredo(&mut self, s: &[u8], f: &mut Visitante<'_>) -> Result<(), ErroMascara> {
        if s.is_empty() {
            return Ok(());
        }
        f(s)?;
        for alfabeto in [B64_PADRAO, B64_URL] {
            self.tmp.clear();
            base64(s, alfabeto, true, &mut self.tmp);
            f(&self.tmp)?;
            let sem_igual = self
                .tmp
                .iter()
                .rposition(|b| *b != b'=')
                .map_or(0, |i| i + 1);
            f(&self.tmp[..sem_igual])?;
        }
        // Miolo do base64 em cada alinhamento: os caracteres que só dependem dos bytes
        // do segredo, para achá-lo dentro de um base64 maior (`usuario:token`, JSON…).
        if s.len() >= TRUSTED_MINIMUM {
            for desloc in 0..3usize {
                self.tmp2.clear();
                reservar(&mut self.tmp2, desloc + s.len());
                self.tmp2.extend(core::iter::repeat_n(0u8, desloc));
                self.tmp2.extend_from_slice(s);
                let ini = (8 * desloc).div_ceil(6);
                let fim = 8 * (desloc + s.len()) / 6;
                for alfabeto in [B64_PADRAO, B64_URL] {
                    self.tmp.clear();
                    base64(&self.tmp2, alfabeto, false, &mut self.tmp);
                    if fim > ini && fim - ini >= 8 {
                        f(&self.tmp[ini..fim])?;
                    }
                }
            }
        }
        self.formas_de_texto(s, f)
    }

    /// As formas de texto de D37 e D54, cada uma só quando difere da literal (e, no
    /// caso do JSON, do escape padrão).
    fn formas_de_texto(&mut self, s: &[u8], f: &mut Visitante<'_>) -> Result<(), ErroMascara> {
        for barra in [false, true] {
            if json_muda(s, barra) {
                self.tmp.clear();
                json(s, barra, &mut self.tmp);
                f(&self.tmp)?;
            }
        }
        if let Ok(texto) = core::str::from_utf8(s) {
            for modo in [
                JsonEscapado::Go,
                JsonEscapado::Ascii,
                JsonEscapado::Php,
                JsonEscapado::GoAscii,
            ] {
                if json_escapado_muda(texto, modo) {
                    self.tmp.clear();
                    json_escapado(texto, modo, &mut self.tmp);
                    f(&self.tmp)?;
                }
            }
        }
        if s.iter().any(|b| aspas_duplas_escapa(*b)) {
            self.tmp.clear();
            aspas_duplas(s, &mut self.tmp);
            f(&self.tmp)?;
        }
        if s.contains(&b'\'') {
            self.tmp.clear();
            aspas_simples(s, &mut self.tmp);
            f(&self.tmp)?;
        }
        if printf_q_muda(s) {
            self.tmp.clear();
            printf_q(s, &mut self.tmp);
            f(&self.tmp)?;
        }
        Ok(())
    }

    /// Basic auth: base64 de `usuario:senha`.
    fn basic(&mut self, u: &[u8], s: &[u8], f: &mut Visitante<'_>) -> Result<(), ErroMascara> {
        if s.is_empty() {
            return Ok(());
        }
        self.tmp2.clear();
        reservar(&mut self.tmp2, u.len() + 1 + s.len());
        self.tmp2.extend_from_slice(u);
        self.tmp2.push(b':');
        self.tmp2.extend_from_slice(s);
        for alfabeto in [B64_PADRAO, B64_URL] {
            self.tmp.clear();
            base64(&self.tmp2, alfabeto, true, &mut self.tmp);
            f(&self.tmp)?;
            let sem_igual = self
                .tmp
                .iter()
                .rposition(|b| *b != b'=')
                .map_or(0, |i| i + 1);
            f(&self.tmp[..sem_igual])?;
        }
        Ok(())
    }

    fn anexo(&mut self, c: &[u8], f: &mut Visitante<'_>) -> Result<(), ErroMascara> {
        if c.is_empty() {
            return Ok(());
        }
        f(c)?;
        f(sem_quebra_final(c))?;
        for alfabeto in [B64_PADRAO, B64_URL] {
            self.tmp.clear();
            base64(c, alfabeto, true, &mut self.tmp);
            f(&self.tmp)?;
            let sem_igual = self
                .tmp
                .iter()
                .rposition(|b| *b != b'=')
                .map_or(0, |i| i + 1);
            f(&self.tmp[..sem_igual])?;
        }
        self.linhas(c, f)
    }

    /// A passada por linha (D11, D34, D36, D54), dos anexos e de todo segredo de várias
    /// linhas: cada linha com [`TRUSTED_MINIMUM`] caracteres ou mais, sem o `\r` do fim
    /// e também aparada (a saída com CRLF do pty, `head`, `grep -v`, `sed -n` pegam
    /// linhas soltas), com as formas de texto dela (`json.dumps(linhas)`, `declare -p`
    /// de uma linha), sem base64 nem miolos (D54: o base64 de linha solta fica em RA3);
    /// os campos de uma linha com tab (`cut -f`); e o valor de `CHAVE=valor`,
    /// `"chave": "valor"` e `chave: valor`, com o mesmo mínimo e as variações de um
    /// segredo (`cut -d=`, `jq -r`, `yq`).
    fn linhas(&mut self, c: &[u8], f: &mut Visitante<'_>) -> Result<(), ErroMascara> {
        for linha in c.split(|b| *b == b'\n') {
            let linha = linha.strip_suffix(b"\r").unwrap_or(linha);
            if caracteres(linha) >= TRUSTED_MINIMUM {
                // As formas de texto e os campos de tab só valem para linha que é texto
                // (UTF-8 válido): uma linha de anexo binário não passa por `json.dumps`,
                // `printf %q` nem `cut -f` sem virar outra coisa, e com elas um binário
                // de 256 KiB passaria do orçamento de D29.
                let texto = core::str::from_utf8(linha).is_ok();
                f(linha)?;
                if texto {
                    self.formas_de_texto(linha, f)?;
                }
                let aparada = linha.trim_ascii();
                if aparada.len() != linha.len() && caracteres(aparada) >= TRUSTED_MINIMUM {
                    f(aparada)?;
                    if texto {
                        self.formas_de_texto(aparada, f)?;
                    }
                }
                // D54: cada campo de uma linha de texto com tab é um valor de linha.
                if texto && linha.contains(&b'\t') {
                    for campo in linha.split(|b| *b == b'\t') {
                        self.valor_da_linha(campo.trim_ascii(), f)?;
                    }
                }
                // D61: na linha que não é texto, o campo de tab só pelo literal (e pelo
                // percent, que o autômato lê em toda forma), aparado e cru, 6+ bytes,
                // quando difere da linha e da linha aparada.
                if !texto && linha.contains(&b'\t') {
                    for campo in linha.split(|b| *b == b'\t') {
                        let aparado = campo.trim_ascii();
                        let cru = (aparado.len() != campo.len()).then_some(campo);
                        for c in core::iter::once(aparado).chain(cru) {
                            if c.len() >= TRUSTED_MINIMUM && c != linha && c != aparada {
                                f(c)?;
                            }
                        }
                    }
                }
                // D34: o valor só vira padrão próprio com o mesmo mínimo das linhas;
                // sem isso, `PORT=8080` mascararia todo `8080` da saída. A linha inteira
                // continua mascarada (acima).
                if let Some(valor) = valor_de_chave(linha) {
                    self.valor_da_linha(valor, f)?;
                    self.comentario_e_aspas(valor, false, f)?;
                }
                let dois = valor_de_dois_pontos(linha);
                if let Some(valor) = dois {
                    self.valor_da_linha(valor, f)?;
                    self.comentario_e_aspas(valor, true, f)?;
                }
                if let Some(valor) = valor_de_chave_composta(linha)
                    && Some(valor) != dois
                {
                    self.valor_da_linha(valor, f)?;
                    self.comentario_e_aspas(valor, true, f)?;
                }
                // D61: os pares `"chave": "string"` em qualquer posição da linha.
                if texto {
                    self.json_da_linha(linha, dois.and_then(sem_aspas), f)?;
                }
            }
        }
        Ok(())
    }

    /// O valor de uma linha de chave e valor, com e sem aspas, se tiver o mínimo.
    fn valor_da_linha(&mut self, valor: &[u8], f: &mut Visitante<'_>) -> Result<(), ErroMascara> {
        if caracteres(valor) >= TRUSTED_MINIMUM {
            self.segredo(valor, f)?;
        }
        if let Some(sem_aspas) = sem_aspas(valor)
            && caracteres(sem_aspas) >= TRUSTED_MINIMUM
        {
            self.segredo(sem_aspas, f)?;
        }
        Ok(())
    }
}

fn sem_quebra_final(c: &[u8]) -> &[u8] {
    let c = c.strip_suffix(b"\n").unwrap_or(c);
    c.strip_suffix(b"\r").unwrap_or(c)
}

/// Caracteres de UTF-8 válido; bytes se não for texto.
fn caracteres(b: &[u8]) -> usize {
    match core::str::from_utf8(b) {
        Ok(s) => s.chars().count(),
        Err(_) => b.len(),
    }
}

/// O valor de uma linha `CHAVE=valor` (ou `export CHAVE=valor`), aparado.
fn valor_de_chave(linha: &[u8]) -> Option<&[u8]> {
    let linha = linha.trim_ascii();
    let linha = linha.strip_prefix(b"export ").unwrap_or(linha);
    let igual = linha.iter().position(|b| *b == b'=')?;
    let chave = linha[..igual].trim_ascii();
    let primeiro = *chave.first()?;
    if primeiro.is_ascii_digit()
        || !chave
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
    {
        return None;
    }
    let valor = linha[igual + 1..].trim_ascii();
    (!valor.is_empty()).then_some(valor)
}

/// O valor de uma linha `"chave": "valor"` (JSON, com ou sem a vírgula do fim, também
/// colado `"k":"v"`, com a chave de qualquer caractere menos `"`) ou `chave: valor`
/// (YAML, também como item `- chave: valor`, `chave : valor` e `chave:\tvalor`),
/// aparado. A chave do YAML é `[A-Za-z0-9_.-]` (com espaço ou tab antes dos dois
/// pontos) e precisa de espaço, tab (ou fim) depois deles, para não confundir com
/// `https://…` ou `host:porta`.
fn valor_de_dois_pontos(linha: &[u8]) -> Option<&[u8]> {
    let l = linha.trim_ascii();
    let l = l.strip_prefix(b"- ").map_or(l, <[u8]>::trim_ascii_start);
    let resto = if let Some(depois) = l.strip_prefix(b"\"") {
        let fim = depois.iter().position(|b| *b == b'"')?;
        if fim == 0 {
            return None;
        }
        depois[fim + 1..].trim_ascii_start().strip_prefix(b":")?
    } else {
        let p = l.iter().position(|b| *b == b':')?;
        let chave = l[..p].trim_ascii_end();
        if chave.is_empty()
            || !chave
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        {
            return None;
        }
        let r = &l[p + 1..];
        if !r.is_empty() && !matches!(r[0], b' ' | b'\t') {
            return None;
        }
        r
    };
    let mut v = resto.trim_ascii();
    if let Some(sem_virgula) = v.strip_suffix(b",") {
        v = sem_virgula.trim_ascii_end();
    }
    (!v.is_empty()).then_some(v)
}

/// Chave YAML composta (D61): `private key: v`, `aws/secret: v`. Depois do recuo (e do
/// `- ` de item), a chave vai até o primeiro `:` seguido de espaço, tab ou fim, sem `#`
/// nem aspas, e não começa por `-`, `{` nem `[`. O valor sai como no
/// [`valor_de_dois_pontos`] (aparado, sem a vírgula do fim).
fn valor_de_chave_composta(linha: &[u8]) -> Option<&[u8]> {
    let l = linha.trim_ascii();
    let l = l.strip_prefix(b"- ").map_or(l, <[u8]>::trim_ascii_start);
    let p = (0..l.len())
        .find(|&p| l[p] == b':' && l.get(p + 1).is_none_or(|b| matches!(b, b' ' | b'\t')))?;
    let chave = &l[..p];
    let primeiro = *chave.first()?;
    if matches!(primeiro, b'-' | b'{' | b'[')
        || chave.iter().any(|b| matches!(b, b'#' | b'"' | b'\''))
    {
        return None;
    }
    let mut v = l[p + 1..].trim_ascii();
    if let Some(sem_virgula) = v.strip_suffix(b",") {
        v = sem_virgula.trim_ascii_end();
    }
    (!v.is_empty()).then_some(v)
}

/// Valor sem aspas com comentário no fim (D61): o que vem antes do primeiro espaço ou
/// tab seguido de `#`, aparado.
fn antes_de_comentario(v: &[u8]) -> Option<&[u8]> {
    let k = (1..v.len()).find(|&k| v[k] == b'#' && matches!(v[k - 1], b' ' | b'\t'))?;
    let c = v[..k].trim_ascii_end();
    (!c.is_empty()).then_some(c)
}

/// Valor entre aspas seguido de espaços e `# …` (D61): o miolo das aspas, até a primeira
/// aspa igual à de abertura seguida de espaços ou tabs e, depois deles, `#`.
fn miolo_antes_de_comentario(v: &[u8]) -> Option<&[u8]> {
    let q = *v.first()?;
    let k = (1..v.len()).find(|&k| {
        v[k] == q
            && matches!(v.get(k + 1), Some(b' ' | b'\t'))
            && v.get(pular_branco_json(v, k + 1)) == Some(&b'#')
    })?;
    Some(&v[1..k])
}

/// Espaço e tab entre os elementos de um par JSON na linha.
fn pular_branco_json(l: &[u8], mut i: usize) -> usize {
    while matches!(l.get(i), Some(b' ' | b'\t')) {
        i += 1;
    }
    i
}

/// Fim (depois da aspa de fechamento) da string JSON que começa na aspa em `ini`. `\`
/// e o byte seguinte andam juntos. `None` se a string não fecha na linha.
fn fim_string_json(l: &[u8], ini: usize) -> Option<usize> {
    let mut p = ini + 1;
    while p < l.len() {
        match l[p] {
            b'\\' => p += 2,
            b'"' => return Some(p + 1),
            _ => p += 1,
        }
    }
    None
}

fn hex4_json(b: &[u8], i: usize) -> Option<u32> {
    b.get(i..i + 4)?
        .iter()
        .try_fold(0u32, |n, c| Some((n << 4) | u32::from(hex(*c)?)))
}

/// Decodifica o miolo de uma string JSON (D61): `\"`, `\\`, `\/`, `\b\f\n\r\t` e
/// `\uXXXX` (com par substituto). Escape inválido ou substituto sozinho: `false` (o par
/// é descartado). Os outros bytes passam iguais.
fn decodificar_string_json(raw: &[u8], saida: &mut Zeroizing<Vec<u8>>) -> bool {
    reservar(saida, raw.len());
    let mut utf8 = [0u8; 4];
    let mut k = 0usize;
    let mut ok = true;
    while k < raw.len() {
        let b = raw[k];
        if b != b'\\' {
            saida.push(b);
            k += 1;
            continue;
        }
        let curto = match raw.get(k + 1) {
            Some(b'"') => b'"',
            Some(b'\\') => b'\\',
            Some(b'/') => b'/',
            Some(b'b') => 0x08,
            Some(b'f') => 0x0c,
            Some(b'n') => b'\n',
            Some(b'r') => b'\r',
            Some(b't') => b'\t',
            Some(b'u') => {
                let n = match hex4_json(raw, k + 2) {
                    Some(alto @ 0xd800..=0xdbff) => {
                        let baixo = if raw.get(k + 6..k + 8) == Some(b"\\u") {
                            hex4_json(raw, k + 8)
                        } else {
                            None
                        };
                        match baixo {
                            Some(baixo @ 0xdc00..=0xdfff) => {
                                k += 12;
                                0x10000 + ((alto - 0xd800) << 10) + (baixo - 0xdc00)
                            }
                            _ => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    Some(n @ (0..=0xd7ff | 0xe000..=0xffff)) => {
                        k += 6;
                        n
                    }
                    _ => {
                        ok = false;
                        break;
                    }
                };
                let Some(c) = char::from_u32(n) else {
                    ok = false;
                    break;
                };
                saida.extend_from_slice(c.encode_utf8(&mut utf8).as_bytes());
                continue;
            }
            _ => {
                ok = false;
                break;
            }
        };
        saida.push(curto);
        k += 2;
    }
    utf8.zeroize();
    ok
}

fn sem_aspas(v: &[u8]) -> Option<&[u8]> {
    let (&a, resto) = v.split_first()?;
    let (&z, meio) = resto.split_last()?;
    (a == z && matches!(a, b'"' | b'\'') && !meio.is_empty()).then_some(meio)
}

fn json_escapa(b: u8, barra: bool) -> bool {
    b < 0x20 || b == b'"' || b == b'\\' || (barra && b == b'/')
}

fn json_muda(s: &[u8], barra: bool) -> bool {
    s.iter().any(|b| json_escapa(*b, barra))
}

/// O valor como fica dentro de uma string JSON (`barra`: também `\/`, como no PHP).
fn json(s: &[u8], barra: bool, saida: &mut Zeroizing<Vec<u8>>) {
    reservar(saida, s.len() * 6);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for &b in s {
        if !json_escapa(b, barra) {
            saida.push(b);
            continue;
        }
        saida.push(b'\\');
        match b {
            b'"' | b'\\' | b'/' => saida.push(b),
            b'\n' => saida.push(b'n'),
            b'\r' => saida.push(b'r'),
            b'\t' => saida.push(b't'),
            0x08 => saida.push(b'b'),
            0x0c => saida.push(b'f'),
            _ => {
                saida.extend_from_slice(b"u00");
                saida.push(HEX[usize::from(b >> 4)]);
                saida.push(HEX[usize::from(b & 15)]);
            }
        }
    }
}

/// As formas de JSON de D37, além do escape padrão de [`json`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum JsonEscapado {
    /// `encoding/json` do Go: `&`, `<`, `>`, U+2028 e U+2029 também como `\uXXXX`.
    Go,
    /// `json.dumps` do Python com `ensure_ascii` (o padrão): todo caractere fora do
    /// ASCII visível e do espaço como `\uXXXX` (par substituto acima de U+FFFF).
    Ascii,
    /// `json_encode` do PHP (o padrão): `/` como `\/` e todo caractere fora do ASCII
    /// como `\uXXXX` (D54). Só quando o valor tem os dois; com um só, o JSON com `\/`
    /// ou o `ensure_ascii` já cobrem.
    Php,
    /// JSON "html-safe" com `ensure_ascii` (D58): o `ensure_ascii` mais `&`, `<` e `>`
    /// como `\uXXXX`. Só quando o valor tem `&`, `<` ou `>` e caractere fora do ASCII;
    /// com um só, o Go ou o `ensure_ascii` já cobrem (U+2028 e U+2029 já saem em `\u`
    /// no `ensure_ascii`, então sozinhos não mudam nada aqui).
    GoAscii,
}

/// O caractere vai como `\uXXXX` nesta forma (fora os escapes curtos do JSON padrão).
fn json_escapado_u(c: char, modo: JsonEscapado) -> bool {
    match modo {
        JsonEscapado::Go => matches!(c, '&' | '<' | '>' | '\u{2028}' | '\u{2029}'),
        JsonEscapado::Ascii => !c.is_ascii() || c == '\u{7f}',
        JsonEscapado::Php => !c.is_ascii(),
        JsonEscapado::GoAscii => !c.is_ascii() || matches!(c, '\u{7f}' | '&' | '<' | '>'),
    }
}

/// A forma difere do escape padrão (que já é um padrão próprio quando muda algo).
fn json_escapado_muda(s: &str, modo: JsonEscapado) -> bool {
    match modo {
        JsonEscapado::Php => s.contains('/') && !s.is_ascii(),
        JsonEscapado::GoAscii => s.contains(['&', '<', '>']) && !s.is_ascii(),
        _ => s.chars().any(|c| json_escapado_u(c, modo)),
    }
}

/// O valor como o Go ou o Python o escrevem dentro de uma string JSON. Hex em
/// minúsculas, como os dois fazem (o hex em maiúsculas fica em RA3).
fn json_escapado(s: &str, modo: JsonEscapado, saida: &mut Zeroizing<Vec<u8>>) {
    // Pior caso: um byte de controle vira 6; um caractere de 4 bytes vira 12.
    reservar(saida, s.len() * 6);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let u = |saida: &mut Zeroizing<Vec<u8>>, n: u16| {
        saida.extend_from_slice(b"\\u");
        for desloc in [12, 8, 4, 0] {
            saida.push(HEX[usize::from((n >> desloc) & 15)]);
        }
    };
    // Os pedaços do caractere passam por estes dois buffers da pilha, zerados no fim.
    let mut utf16 = [0u16; 2];
    let mut utf8 = [0u8; 4];
    for c in s.chars() {
        if json_escapado_u(c, modo) {
            for n in c.encode_utf16(&mut utf16).iter() {
                u(saida, *n);
            }
            continue;
        }
        let bytes = c.encode_utf8(&mut utf8).as_bytes();
        let barra = modo == JsonEscapado::Php;
        if bytes.len() == 1 && json_escapa(bytes[0], barra) {
            json(bytes, barra, saida);
        } else {
            saida.extend_from_slice(bytes);
        }
    }
    utf16.zeroize();
    utf8.zeroize();
}

/// Precisa de `\` antes entre aspas duplas do shell (`declare -p`, `export -p`).
fn aspas_duplas_escapa(b: u8) -> bool {
    matches!(b, b'$' | b'`' | b'"' | b'\\')
}

/// O valor como o bash o escreve entre aspas duplas.
fn aspas_duplas(s: &[u8], saida: &mut Zeroizing<Vec<u8>>) {
    reservar(saida, s.len() * 2);
    for &b in s {
        if aspas_duplas_escapa(b) {
            saida.push(b'\\');
        }
        saida.push(b);
    }
}

/// O valor como o bash o escreve entre aspas simples (`${X@Q}`, `set`, `set -x`): cada
/// `'` vira `'\''` (D54). As aspas das pontas ficam fora do padrão.
fn aspas_simples(s: &[u8], saida: &mut Zeroizing<Vec<u8>>) {
    reservar(saida, s.len() * 4);
    for &b in s {
        if b == b'\'' {
            saida.extend_from_slice(b"'\\''");
        } else {
            saida.push(b);
        }
    }
}

/// `printf %q` do bash 5.2 põe `\` antes deste caractere na posição `i` de `s` (D54,
/// conferido contra o bash real em `v2_56_printf_q_igual_ao_bash`): os especiais do
/// shell sempre; `#` só no começo; `~` no começo ou depois de `=` ou `:`.
fn printf_q_escapa(s: &[u8], i: usize) -> bool {
    match s[i] {
        b' ' | b'!' | b'"' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b',' | b';' | b'<'
        | b'>' | b'?' | b'[' | b'\\' | b']' | b'^' | b'`' | b'{' | b'|' | b'}' => true,
        b'#' => i == 0,
        b'~' => i == 0 || matches!(s[i - 1], b'=' | b':'),
        _ => false,
    }
}

/// O `printf %q` só vale aqui para valor em ASCII visível e espaço: com controle ou
/// fora do ASCII o bash troca para `$'…'` (RA3).
fn printf_q_muda(s: &[u8]) -> bool {
    s.iter().all(|b| (0x20..=0x7e).contains(b)) && (0..s.len()).any(|i| printf_q_escapa(s, i))
}

/// O valor como o `printf %q` do bash o escreve.
fn printf_q(s: &[u8], saida: &mut Zeroizing<Vec<u8>>) {
    reservar(saida, s.len() * 2);
    for (i, &b) in s.iter().enumerate() {
        if printf_q_escapa(s, i) {
            saida.push(b'\\');
        }
        saida.push(b);
    }
}

// ---------------------------------------------------------------------------------
// Fluxo
// ---------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Modo {
    /// Os bytes como vieram.
    Cru,
    /// `%XX` (qualquer caixa) decodificado.
    Percent,
    /// `%XX` e `+` → espaço (formulário).
    Formulario,
}

/// `%` (e talvez um dígito hex) esperando o resto.
#[derive(Clone, Copy)]
enum Escape {
    Nada,
    Percent { ini: u64 },
    Hex { ini: u64, alto: u8, char_alto: u8 },
}

/// Uma leitura da saída: decodifica e anda no autômato.
struct Visao {
    modo: Modo,
    estado: u32,
    /// Posição original do começo de cada byte decodificado ainda em casamento (as
    /// últimas `profundidade` posições). Não são segredo: só deslocamentos.
    posicoes: VecDeque<u64>,
    escape: Escape,
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

impl Visao {
    fn new(modo: Modo) -> Self {
        Visao {
            modo,
            estado: 0,
            posicoes: VecDeque::new(),
            escape: Escape::Nada,
        }
    }

    /// Anda um byte decodificado `[ini, fim)` e devolve o trecho original coberto.
    fn andar(&mut self, a: &Automato, b: u8, ini: u64, fim: u64) -> Option<(u64, u64)> {
        self.estado = a.passo(self.estado, b);
        self.posicoes.push_back(ini);
        let prof = a.profundidade(self.estado);
        while self.posicoes.len() > prof {
            self.posicoes.pop_front();
        }
        let n = a.saida(self.estado);
        if n == 0 {
            return None;
        }
        let inicio = self.posicoes[self.posicoes.len() - n];
        Some((inicio, fim))
    }

    /// Lê o byte original `b` na posição `p`.
    fn ler(&mut self, a: &Automato, b: u8, p: u64, cobrir: &mut dyn FnMut(u64, u64)) {
        let mut andar = |v: &mut Visao, byte: u8, ini: u64, fim: u64| {
            if let Some((i, f)) = v.andar(a, byte, ini, fim) {
                cobrir(i, f);
            }
        };
        if self.modo == Modo::Cru {
            andar(self, b, p, p + 1);
            return;
        }
        match self.escape {
            Escape::Percent { ini } => {
                if let Some(alto) = hex(b) {
                    self.escape = Escape::Hex {
                        ini,
                        alto,
                        char_alto: b,
                    };
                    return;
                }
                self.escape = Escape::Nada;
                andar(self, b'%', ini, ini + 1);
            }
            Escape::Hex {
                ini,
                alto,
                char_alto,
            } => {
                self.escape = Escape::Nada;
                if let Some(baixo) = hex(b) {
                    andar(self, (alto << 4) | baixo, ini, p + 1);
                    return;
                }
                andar(self, b'%', ini, ini + 1);
                andar(self, char_alto, ini + 1, ini + 2);
            }
            Escape::Nada => {}
        }
        // Byte normal (ou o que veio depois de um escape que não fechou).
        if b == b'%' {
            self.escape = Escape::Percent { ini: p };
        } else if b == b'+' && self.modo == Modo::Formulario {
            andar(self, b' ', p, p + 1);
        } else {
            andar(self, b, p, p + 1);
        }
    }

    /// Fim da saída: um escape pela metade vira bytes comuns.
    fn fechar(&mut self, a: &Automato, cobrir: &mut dyn FnMut(u64, u64)) {
        let escape = core::mem::replace(&mut self.escape, Escape::Nada);
        let mut andar = |v: &mut Visao, byte: u8, ini: u64, fim: u64| {
            if let Some((i, f)) = v.andar(a, byte, ini, fim) {
                cobrir(i, f);
            }
        };
        match escape {
            Escape::Nada => {}
            Escape::Percent { ini } => andar(self, b'%', ini, ini + 1),
            Escape::Hex { ini, char_alto, .. } => {
                andar(self, b'%', ini, ini + 1);
                andar(self, char_alto, ini + 1, ini + 2);
            }
        }
    }
}

/// Junta `[ini, fim)` à lista ordenada de trechos cobertos (disjuntos e sem se
/// encostar).
fn cobrir(cobertos: &mut Vec<(u64, u64)>, ini: u64, fim: u64) {
    let mut i = cobertos.len();
    while i > 0 && cobertos[i - 1].1 >= ini {
        i -= 1;
    }
    let mut j = i;
    while j < cobertos.len() && cobertos[j].0 <= fim {
        j += 1;
    }
    let (mut a, mut b) = (ini, fim);
    if j > i {
        a = a.min(cobertos[i].0);
        b = b.max(cobertos[j - 1].1);
    }
    cobertos.splice(i..j, core::iter::once((a, b)));
}

/// A máscara de um fluxo de saída (stdout ou stderr). Sem `Clone`; zera ao sair.
pub struct Fluxo<'m> {
    mascara: &'m Mascara,
    /// Bytes originais de `emitido` a `lido`: só o pedaço em trabalho (esvazia ao fim
    /// de cada [`Fluxo::mascarar`]).
    pendente: Zeroizing<Vec<u8>>,
    emitido: u64,
    lido: u64,
    visoes: [Visao; 3],
    cobertos: Vec<(u64, u64)>,
    /// O último byte devolvido estava coberto (um trecho que continua não repete a
    /// marca).
    em_marca: bool,
}

impl ZeroizeOnDrop for Fluxo<'_> {}

impl fmt::Debug for Fluxo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Fluxo(••••)")
    }
}

impl Fluxo<'_> {
    /// Recebe mais um pedaço da saída e acrescenta a `saida` o que já é seguro
    /// devolver, mascarado.
    pub fn mascarar(&mut self, pedaco: &[u8], saida: &mut Vec<u8>) {
        reservar(&mut self.pendente, pedaco.len());
        self.pendente.extend_from_slice(pedaco);
        let a = &self.mascara.automato;
        let cobertos = &mut self.cobertos;
        let mut cobre = |i: u64, f: u64| cobrir(cobertos, i, f);
        for &b in pedaco {
            for v in &mut self.visoes {
                v.ler(a, b, self.lido, &mut cobre);
            }
            self.lido += 1;
        }
        // D34 revisto: nada fica segurado; o pedaço inteiro sai agora.
        let lido = self.lido;
        self.emitir_ate(lido, saida);
    }

    /// Fim da saída. Nada ficou segurado (D34 revisto): um `%X` pela metade no fim já
    /// saiu como veio; aqui só fecha as visões e zera o fluxo.
    pub fn terminar(mut self, saida: &mut Vec<u8>) {
        let a = &self.mascara.automato;
        let cobertos = &mut self.cobertos;
        let mut cobre = |i: u64, f: u64| cobrir(cobertos, i, f);
        for v in &mut self.visoes {
            v.fechar(a, &mut cobre);
        }
        let lido = self.lido;
        self.emitir_ate(lido, saida);
    }

    fn emitir_ate(&mut self, seguro: u64, saida: &mut Vec<u8>) {
        debug_assert!(seguro >= self.emitido && seguro <= self.lido);
        let seguro = seguro.clamp(self.emitido, self.lido);
        let base = self.emitido;
        let mut cursor = base;
        let puxar = |de: u64, ate: u64, saida: &mut Vec<u8>, pendente: &[u8]| {
            let (de, ate) = ((de - base) as usize, (ate - base) as usize);
            saida.extend_from_slice(&pendente[de..ate]);
        };
        let mut usados = 0;
        for &(ini, fim) in &self.cobertos {
            if ini >= seguro {
                break;
            }
            // Um trecho que começou num pedaço anterior (segredo quebrado entre pedaços):
            // o que já saiu fica como saiu; daqui para a frente, marca (D34 revisto).
            let ini = ini.max(cursor);
            if ini > cursor {
                puxar(cursor, ini, saida, &self.pendente);
                self.em_marca = false;
            }
            let fim_aqui = fim.min(seguro);
            if fim_aqui > ini {
                if !self.em_marca {
                    saida.extend_from_slice(MARCA.as_bytes());
                    self.em_marca = true;
                }
                cursor = fim_aqui;
            }
            if fim <= seguro {
                usados += 1;
            } else {
                break;
            }
        }
        if seguro > cursor {
            puxar(cursor, seguro, saida, &self.pendente);
            self.em_marca = false;
        }
        self.cobertos.drain(..usados);
        // Tira da frente o que saiu e zera a sobra no fim da área usada.
        let n = (seguro - base) as usize;
        let total = self.pendente.len();
        self.pendente.copy_within(n.., 0);
        self.pendente[total - n..].zeroize();
        self.pendente.truncate(total - n);
        self.emitido = seguro;
    }
}
