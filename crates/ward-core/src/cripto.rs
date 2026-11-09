//! Primitivas do formato, só por bibliotecas auditadas (S6, V2.7).
//!
//! Este módulo **não** implementa cripto: só chama `argon2` (Argon2id, RFC 9106),
//! `chacha20poly1305` (XChaCha20-Poly1305, `draft-irtf-cfrg-xchacha`) e `getrandom`
//! (aleatório do sistema). A comparação da tag é feita dentro do `chacha20poly1305`.
//! O que fica aqui é a cola: tamanhos fixos, buffers que zeram ao sair e um erro
//! único que não diz o que falhou.
//!
//! O formato (`crate::formato`) usa só estas funções; os vetores conhecidos (V2.5)
//! passam por elas também.

use core::fmt;

use argon2::password_hash::phc::{Output, ParamsString, PasswordHash, Salt};
use argon2::{ARGON2ID_IDENT, Algorithm, Argon2, AssociatedData, Block, ParamsBuilder, Version};
use chacha20poly1305::aead::AeadInOut;
use chacha20poly1305::{Key, KeyInit, Tag, XChaCha20Poly1305, XNonce};
use zeroize::{ZeroizeOnDrop, Zeroizing};

use crate::formato::ParametrosKdf;
use crate::limites::MAX_HASH_PIN;

/// Tamanho da chave (KEK derivada e chave de dados), em bytes.
pub const TAM_CHAVE: usize = 32;
/// Tamanho do nonce do XChaCha20-Poly1305, em bytes.
pub const TAM_NONCE: usize = 24;
/// Tamanho da tag do Poly1305, em bytes.
pub const TAM_TAG: usize = 16;

/// Chave simétrica de 256 bits (a chave de dados e a KEK) que zera ao sair. Sem
/// `Debug`, `Display` nem `Clone`.
///
/// Fica no heap (D40): por dentro é um `Box<Zeroizing<[u8; 32]>>`. Mover uma `Chave`
/// passa só o ponteiro, e os 32 bytes vivem num bloco só, que zera ao liberar (o
/// alocador de teste do V2.33 o enxerga). Um `Zeroizing<[u8; 32]>` por valor deixaria
/// uma cópia na pilha a cada movimento, que ninguém zera. O tipo próprio (em vez de um
/// apelido) existe para declarar o [`ZeroizeOnDrop`] (V2.28), que o `zeroize` não dá a
/// um `Box` qualquer.
pub(crate) struct Chave(Box<Zeroizing<[u8; TAM_CHAVE]>>);

// D40, V2.46: a `Chave` tem o tamanho de um ponteiro (os bytes nunca vão por valor).
const _: () = assert!(core::mem::size_of::<Chave>() == core::mem::size_of::<usize>());

// O único campo é um `Zeroizing` no heap: zera ao sair (V2.28).
impl ZeroizeOnDrop for Chave {}

impl core::ops::Deref for Chave {
    type Target = [u8; TAM_CHAVE];

    fn deref(&self) -> &[u8; TAM_CHAVE] {
        &self.0
    }
}

impl core::ops::DerefMut for Chave {
    fn deref_mut(&mut self) -> &mut [u8; TAM_CHAVE] {
        &mut self.0
    }
}

impl Chave {
    /// Chave zerada, já no heap: o segredo é escrito depois, no lugar onde vai ficar.
    /// (Os zeros do `Box::new` não são segredo.)
    fn zerada() -> Self {
        Self(Box::new(Zeroizing::new([0u8; TAM_CHAVE])))
    }

    /// Chave copiada de `bytes` (o que o `abrir` decifrou) direto para o heap.
    pub(crate) fn de_bytes(bytes: &[u8]) -> Result<Self, FalhaCripto> {
        if bytes.len() != TAM_CHAVE {
            return Err(FalhaCripto);
        }
        let mut chave = Self::zerada();
        chave.copy_from_slice(bytes);
        Ok(chave)
    }
}

/// Falha de cripto. Não diz se foi a tag, o tamanho ou o parâmetro: quem chama
/// transforma em erro genérico do formato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FalhaCripto;

/// Argon2id v0x13 (RFC 9106) com todas as entradas do RFC. O formato chama com
/// `segredo` e `dados_assoc` vazios; os vetores do RFC (V2.5) passam por aqui com eles
/// preenchidos.
///
/// A memória de trabalho é alocada aqui e zerada ao sair (o `argon2` 0.6 não zera a
/// memória que ele mesmo aloca em `hash_password_into`).
pub(crate) fn argon2id(
    senha: &[u8],
    sal: &[u8],
    segredo: &[u8],
    dados_assoc: &[u8],
    custo: &ParametrosKdf,
    saida: &mut [u8],
) -> Result<(), FalhaCripto> {
    let mut construtor = ParamsBuilder::new();
    construtor
        .m_cost(custo.memoria_kib)
        .t_cost(custo.passes)
        .p_cost(custo.paralelismo)
        .output_len(saida.len());
    if !dados_assoc.is_empty() {
        construtor.data(AssociatedData::new(dados_assoc).map_err(|_| FalhaCripto)?);
    }
    let parametros = construtor.build().map_err(|_| FalhaCripto)?;
    let contexto = if segredo.is_empty() {
        Argon2::new(Algorithm::Argon2id, Version::V0x13, parametros)
    } else {
        Argon2::new_with_secret(segredo, Algorithm::Argon2id, Version::V0x13, parametros)
            .map_err(|_| FalhaCripto)?
    };
    let blocos = contexto.params().block_count();
    let mut memoria: Zeroizing<Vec<Block>> = Zeroizing::new(Vec::new());
    memoria.try_reserve_exact(blocos).map_err(|_| FalhaCripto)?;
    memoria.resize(blocos, Block::new());
    let resultado = contexto.hash_password_into_with_memory(senha, sal, saida, &mut memoria[..]);
    if resultado.is_err() {
        zeroize::Zeroize::zeroize(saida);
        return Err(FalhaCripto);
    }
    Ok(())
}

/// Deriva a KEK de 32 bytes da senha mestra (Argon2id sem segredo nem dados
/// associados). Os limites de parâmetro são conferidos pelo formato antes.
pub(crate) fn derivar_kek(
    senha: &[u8],
    sal: &[u8],
    custo: &ParametrosKdf,
) -> Result<Chave, FalhaCripto> {
    let mut kek = Chave::zerada();
    argon2id(senha, sal, &[], &[], custo, &mut kek[..])?;
    Ok(kek)
}

fn cifra(chave: &[u8; TAM_CHAVE]) -> Result<XChaCha20Poly1305, FalhaCripto> {
    // Referência à chave sem cópia; a cópia dentro do `XChaCha20Poly1305` zera no drop
    // (feature `zeroize`).
    let chave = <&Key>::try_from(&chave[..]).map_err(|_| FalhaCripto)?;
    Ok(XChaCha20Poly1305::new(chave))
}

/// XChaCha20-Poly1305: cifra `texto` com `aad` autenticado. Saída = cifrado ‖ tag.
///
/// O buffer de saída é alocado do tamanho final antes de receber o texto, então não
/// há realocação que deixe cópia em claro na memória liberada.
pub(crate) fn selar(
    chave: &[u8; TAM_CHAVE],
    nonce: &[u8; TAM_NONCE],
    aad: &[u8],
    texto: &[u8],
) -> Result<Vec<u8>, FalhaCripto> {
    let total = texto.len().checked_add(TAM_TAG).ok_or(FalhaCripto)?;
    let nonce = <&XNonce>::try_from(&nonce[..]).map_err(|_| FalhaCripto)?;
    let cifra = cifra(chave)?;
    let mut saida: Vec<u8> = Vec::new();
    saida.try_reserve_exact(total).map_err(|_| FalhaCripto)?;
    saida.extend_from_slice(texto);
    match cifra.encrypt_inout_detached(nonce, aad, (&mut saida[..]).into()) {
        Ok(tag) => {
            saida.extend_from_slice(tag.as_slice());
            Ok(saida)
        }
        Err(_) => {
            zeroize::Zeroize::zeroize(&mut saida);
            Err(FalhaCripto)
        }
    }
}

/// XChaCha20-Poly1305: confere a tag (dentro do `chacha20poly1305`) e decifra. O texto
/// em claro só existe num buffer que zera ao sair, e só é devolvido se a tag bater.
pub(crate) fn abrir(
    chave: &[u8; TAM_CHAVE],
    nonce: &[u8; TAM_NONCE],
    aad: &[u8],
    cifrado: &[u8],
) -> Result<Zeroizing<Vec<u8>>, FalhaCripto> {
    let corpo = cifrado.len().checked_sub(TAM_TAG).ok_or(FalhaCripto)?;
    let (msg, tag) = cifrado.split_at(corpo);
    let tag = <&Tag>::try_from(tag).map_err(|_| FalhaCripto)?;
    let nonce = <&XNonce>::try_from(&nonce[..]).map_err(|_| FalhaCripto)?;
    let cifra = cifra(chave)?;
    let mut texto: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::new());
    texto.try_reserve_exact(corpo).map_err(|_| FalhaCripto)?;
    texto.extend_from_slice(msg);
    cifra
        .decrypt_inout_detached(nonce, aad, (&mut texto[..]).into(), tag)
        .map_err(|_| FalhaCripto)?;
    Ok(texto)
}

/// Bytes aleatórios do sistema operacional (`getrandom`). Falha em vez de seguir com
/// aleatório fraco.
pub(crate) fn aleatorio(destino: &mut [u8]) -> Result<(), FalhaCripto> {
    getrandom::fill(destino).map_err(|_| FalhaCripto)
}

/// Chave aleatória nova (chave de dados).
pub(crate) fn chave_aleatoria() -> Result<Chave, FalhaCripto> {
    let mut chave = Chave::zerada();
    aleatorio(&mut chave[..])?;
    Ok(chave)
}

// ---- Hash do PIN (D30) ----
//
// O hash é calculado por [`argon2id`] (memória de trabalho zerada, nota da Fase 13), e
// o texto PHC é montado e lido pelos tipos do `phc` (o mesmo do `argon2`). A comparação
// do hash é o `CtEq` do `ctutils`, em tempo constante (D40). Nada disso é feito à mão.
// Os tipos `Copy` do `phc` (`Output`, `Salt`) só servem para ler e escrever o texto PHC
// guardado, que já é o conteúdo do `HashPin`; a saída recalculada fica num `Zeroizing`.

/// Sal do hash do PIN, em bytes (o recomendado pelo PHC).
pub(crate) const TAM_SAL_PIN: usize = 16;
/// Saída do hash do PIN, em bytes.
pub(crate) const TAM_HASH_PIN: usize = 32;

/// Hash do PIN já lido do texto PHC: parâmetros, sal e saída. Não sai do crate.
pub(crate) struct PhcPin {
    pub(crate) custo: ParametrosKdf,
    sal: Salt,
    hash: Output,
}

/// Escreve num buffer fixo, sem alocar; recusa o que não cabe.
struct BufferFixo<'b> {
    buf: &'b mut [u8],
    usados: usize,
}

impl fmt::Write for BufferFixo<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let fim = self
            .usados
            .checked_add(s.len())
            .filter(|f| *f <= self.buf.len())
            .ok_or(fmt::Error)?;
        self.buf[self.usados..fim].copy_from_slice(s.as_bytes());
        self.usados = fim;
        Ok(())
    }
}

/// Texto PHC canônico de (parâmetros, sal, saída), escrito em `buf`. Devolve quantos
/// bytes usou. Não aloca.
fn escrever_phc(
    custo: &ParametrosKdf,
    sal: &[u8],
    saida: &[u8],
    buf: &mut [u8; MAX_HASH_PIN],
) -> Result<usize, FalhaCripto> {
    let mut params = ParamsString::new();
    for (nome, valor) in [
        ("m", custo.memoria_kib),
        ("t", custo.passes),
        ("p", custo.paralelismo),
    ] {
        params.add_decimal(nome, valor).map_err(|_| FalhaCripto)?;
    }
    let phc = PasswordHash {
        algorithm: ARGON2ID_IDENT,
        version: Some(Version::V0x13.into()),
        params,
        salt: Some(Salt::new(sal).map_err(|_| FalhaCripto)?),
        hash: Some(Output::new(saida).map_err(|_| FalhaCripto)?),
    };
    let mut escritor = BufferFixo { buf, usados: 0 };
    fmt::write(&mut escritor, format_args!("{phc}")).map_err(|_| FalhaCripto)?;
    Ok(escritor.usados)
}

/// Hash Argon2id do PIN com sal aleatório novo, em texto PHC. O texto nasce numa
/// alocação do tamanho exato, que zera ao sair.
pub(crate) fn hash_pin(
    pin: &[u8],
    custo: &ParametrosKdf,
) -> Result<Zeroizing<String>, FalhaCripto> {
    let mut sal = [0u8; TAM_SAL_PIN];
    aleatorio(&mut sal)?;
    let mut saida: Zeroizing<[u8; TAM_HASH_PIN]> = Zeroizing::new([0u8; TAM_HASH_PIN]);
    argon2id(pin, &sal, &[], &[], custo, &mut saida[..])?;
    let mut buf: Zeroizing<[u8; MAX_HASH_PIN]> = Zeroizing::new([0u8; MAX_HASH_PIN]);
    let n = escrever_phc(custo, &sal, &saida[..], &mut buf)?;
    let texto = core::str::from_utf8(&buf[..n]).map_err(|_| FalhaCripto)?;
    let mut s: Zeroizing<String> = Zeroizing::new(String::new());
    s.try_reserve_exact(n).map_err(|_| FalhaCripto)?;
    s.push_str(texto);
    Ok(s)
}

/// Lê um texto PHC de hash do PIN e confere tudo, sem alocar: Argon2id v0x13, só
/// `m`, `t` e `p`, dentro de `[minimo, maximo]`, sal de [`TAM_SAL_PIN`] bytes, saída de
/// [`TAM_HASH_PIN`] bytes e texto canônico (escrito de novo, dá os mesmos bytes). Um
/// hash inflado não faz o Argon2 rodar caro; um rebaixado não roda barato.
pub(crate) fn ler_phc_pin(
    texto: &[u8],
    minimo: &ParametrosKdf,
    maximo: &ParametrosKdf,
) -> Option<PhcPin> {
    if texto.len() > MAX_HASH_PIN || !texto.is_ascii() {
        return None;
    }
    let texto = core::str::from_utf8(texto).ok()?;
    let phc = PasswordHash::new(texto).ok()?;
    if phc.algorithm != ARGON2ID_IDENT || phc.version != Some(Version::V0x13.into()) {
        return None;
    }
    if phc.params.iter().count() != 3 {
        return None;
    }
    let custo = ParametrosKdf {
        memoria_kib: phc.params.get_decimal("m")?,
        passes: phc.params.get_decimal("t")?,
        paralelismo: phc.params.get_decimal("p")?,
    };
    let dentro = |v: u32, min: u32, max: u32| (min..=max).contains(&v);
    if !dentro(custo.memoria_kib, minimo.memoria_kib, maximo.memoria_kib)
        || !dentro(custo.passes, minimo.passes, maximo.passes)
        || !dentro(
            custo.paralelismo,
            minimo.paralelismo.max(1),
            maximo.paralelismo,
        )
        || u64::from(custo.memoria_kib) < 8 * u64::from(custo.paralelismo)
    {
        return None;
    }
    let sal = phc.salt?;
    let hash = phc.hash?;
    if sal.len() != TAM_SAL_PIN || hash.len() != TAM_HASH_PIN {
        return None;
    }
    let mut buf: Zeroizing<[u8; MAX_HASH_PIN]> = Zeroizing::new([0u8; MAX_HASH_PIN]);
    let n = escrever_phc(&custo, &sal, hash.as_bytes(), &mut buf).ok()?;
    if &buf[..n] != texto.as_bytes() {
        return None;
    }
    Some(PhcPin { custo, sal, hash })
}

/// Confere o PIN contra o hash: recalcula por [`argon2id`] com o sal e os parâmetros do
/// hash, num `Zeroizing`, e compara com a saída guardada pelo `CtEq` do `ctutils`
/// (tempo constante, pela biblioteca; D40, V2.7).
pub(crate) fn conferir_pin(pin: &[u8], phc: &PhcPin) -> bool {
    let mut saida: Zeroizing<[u8; TAM_HASH_PIN]> = Zeroizing::new([0u8; TAM_HASH_PIN]);
    if argon2id(pin, &phc.sal, &[], &[], &phc.custo, &mut saida[..]).is_err() {
        return false;
    }
    iguais_em_tempo_constante(&saida[..], phc.hash.as_bytes())
}

/// Igualdade de bytes em tempo constante, pelo `CtEq` do `ctutils` (o mesmo que o `phc`
/// usa). Comprimentos diferentes dão `false`.
pub(crate) fn iguais_em_tempo_constante(a: &[u8], b: &[u8]) -> bool {
    use ctutils::CtEq;
    a.ct_eq(b).to_bool()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn custo(memoria_kib: u32, passes: u32, paralelismo: u32) -> ParametrosKdf {
        ParametrosKdf {
            memoria_kib,
            passes,
            paralelismo,
        }
    }

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// RFC 9106, seção 5.3 (Argon2id, versão 0x13).
    #[test]
    fn v2_5_argon2id_rfc9106() {
        let mut saida = [0u8; 32];
        argon2id(
            &[0x01; 32],
            &[0x02; 16],
            &[0x03; 8],
            &[0x04; 12],
            &custo(32, 3, 4),
            &mut saida,
        )
        .unwrap();
        assert_eq!(
            saida.to_vec(),
            hex("0d 64 0d f5 8d 78 76 6c 08 c0 37 a3 4a 8b 53 c9
                 d0 1e f0 45 2d 75 b6 5e b5 25 20 e9 6b 01 e6 59")
        );
    }

    /// `derivar_kek` é o `argon2id` sem segredo nem dados associados (é o caminho que o
    /// formato usa).
    #[test]
    fn derivar_kek_e_o_argon2id_sem_extras() {
        let mut esperado = [0u8; 32];
        argon2id(
            b"senha",
            &[0x02; 32],
            &[],
            &[],
            &custo(64, 3, 4),
            &mut esperado,
        )
        .unwrap();
        let kek = derivar_kek(b"senha", &[0x02; 32], &custo(64, 3, 4)).unwrap();
        assert_eq!(&kek[..], &esperado[..]);
        assert_ne!(&kek[..], &[0u8; 32][..]);
    }

    // `draft-irtf-cfrg-xchacha-03`, apêndice A.3.1.
    const XC_CHAVE: &str = "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f";
    const XC_NONCE: &str = "404142434445464748494a4b4c4d4e4f5051525354555657";
    const XC_AAD: &str = "50515253c0c1c2c3c4c5c6c7";
    const XC_TEXTO: &[u8] = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
    const XC_CIFRADO: &str = "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb
        731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b452
        2f8c9ba40db5d945b11b69b982c1bb9e3f3fac2bc369488f76b2383565d3fff9
        21f9664c97637da9768812f615c68b13b52e";
    const XC_TAG: &str = "c0875924c1c7987947deafd8780acf49";

    fn xc() -> ([u8; 32], [u8; 24], Vec<u8>, Vec<u8>) {
        let chave: [u8; 32] = hex(XC_CHAVE).try_into().unwrap();
        let nonce: [u8; 24] = hex(XC_NONCE).try_into().unwrap();
        let mut esperado = hex(XC_CIFRADO);
        esperado.extend(hex(XC_TAG));
        (chave, nonce, hex(XC_AAD), esperado)
    }

    #[test]
    fn v2_5_xchacha20poly1305_draft_selar() {
        let (chave, nonce, aad, esperado) = xc();
        assert_eq!(selar(&chave, &nonce, &aad, XC_TEXTO).unwrap(), esperado);
    }

    #[test]
    fn v2_5_xchacha20poly1305_draft_abrir() {
        let (chave, nonce, aad, cifrado) = xc();
        assert_eq!(
            &abrir(&chave, &nonce, &aad, &cifrado).unwrap()[..],
            XC_TEXTO
        );
    }

    /// Ataque: qualquer bit trocado no cifrado, na tag, no AAD ou no nonce faz `abrir`
    /// recusar, e o erro é o mesmo.
    #[test]
    fn abrir_recusa_qualquer_bit_trocado() {
        let (chave, nonce, aad, cifrado) = xc();
        for i in 0..cifrado.len() {
            let mut c = cifrado.clone();
            c[i] ^= 1 << (i % 8);
            assert_eq!(abrir(&chave, &nonce, &aad, &c), Err(FalhaCripto));
        }
        for i in 0..aad.len() {
            let mut a = aad.clone();
            a[i] ^= 0x01;
            assert_eq!(abrir(&chave, &nonce, &a, &cifrado), Err(FalhaCripto));
        }
        for i in 0..nonce.len() {
            let mut n = nonce;
            n[i] ^= 0x80;
            assert_eq!(abrir(&chave, &n, &aad, &cifrado), Err(FalhaCripto));
        }
        let mut k = chave;
        k[0] ^= 0x01;
        assert_eq!(abrir(&k, &nonce, &aad, &cifrado), Err(FalhaCripto));
        for tam in 0..cifrado.len() {
            assert_eq!(
                abrir(&chave, &nonce, &aad, &cifrado[..tam]),
                Err(FalhaCripto)
            );
        }
    }

    #[test]
    fn aleatorio_nao_repete() {
        let a = chave_aleatoria().unwrap();
        let b = chave_aleatoria().unwrap();
        assert_ne!(&a[..], &b[..]);
        assert_ne!(&a[..], &[0u8; 32][..]);
    }
}
