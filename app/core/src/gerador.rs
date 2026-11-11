//! Gerador de senha avulso (Fase 37, tela `Gerador de senha / Avulso`).
//!
//! A senha nasce só em `ward_core::generator` (RNG do sistema, sem viés, símbolos de D35),
//! nunca no JavaScript. Ela fica aqui, no lado Rust, num [`Segredo`] que zera ao sair:
//!
//! - `gerar_senha` devolve à interface só o que não é segredo: o tamanho, a força
//!   estimada pelas opções e o conjunto de símbolos de D35 (para a tela mostrar o mesmo
//!   conjunto que o gerador usa);
//! - `copiar_senha_gerada` manda a senha para a área de transferência pelo mesmo
//!   [`crate::area_transferencia::Copiador`] do "copiar" (S12, D4): a resposta é `()`;
//! - `revelar_senha_gerada` é o único caminho do valor para a webview, e só no clique em
//!   "revelar" que o design mostra (S11). A tela esconde de novo em 30 s ou ao sair;
//! - `descartar_senha_gerada` zera a senha ao sair da tela. Nada é gravado: a senha
//!   avulsa não fica no cofre nem em disco.

use std::fmt;

use ward_core::generator::{self as nucleo, GeneratorOptions};
use zeroize::Zeroizing;

use crate::ErroApp;
use crate::segredo::Segredo;

/// Menor tamanho da tela (o design diz "De 8 a 64"). Fica dentro de
/// `ward_core::generator::MIN_SIZE..=MAX_SIZE`.
pub const TAMANHO_MIN: usize = 8;
/// Maior tamanho da tela.
pub const TAMANHO_MAX: usize = 64;
/// Tamanho inicial (o mesmo do gerador do núcleo e do design: "24 caracteres").
pub const TAMANHO_PADRAO: usize = nucleo::DEFAULT_SIZE;

/// Opções que vieram da interface (não confiável): o tamanho é conferido aqui e de novo
/// no núcleo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Opcoes {
    pub tamanho: u64,
    pub minusculas: bool,
    pub maiusculas: bool,
    pub digitos: bool,
    pub simbolos: bool,
}

impl Opcoes {
    fn validar(self) -> Result<GeneratorOptions, ErroApp> {
        let tamanho = usize::try_from(self.tamanho).map_err(|_| ErroApp::PedidoInvalido)?;
        if !(TAMANHO_MIN..=TAMANHO_MAX).contains(&tamanho) {
            return Err(ErroApp::PedidoInvalido);
        }
        if !(self.minusculas || self.maiusculas || self.digitos || self.simbolos) {
            return Err(ErroApp::PedidoInvalido);
        }
        Ok(GeneratorOptions {
            size: tamanho,
            lowercase: self.minusculas,
            uppercase: self.maiusculas,
            digits: self.digitos,
            symbols: self.simbolos,
        })
    }
}

/// Força pela entropia das opções (não pelo valor gerado, que a interface não vê).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Forca {
    Fraca,
    Razoavel,
    Forte,
    MuitoForte,
}

impl Forca {
    fn de_bits(bits: u32) -> Forca {
        match bits {
            0..60 => Forca::Fraca,
            60..80 => Forca::Razoavel,
            80..128 => Forca::Forte,
            _ => Forca::MuitoForte,
        }
    }

    /// Código estável para a interface.
    pub fn codigo(self) -> &'static str {
        match self {
            Forca::Fraca => "fraca",
            Forca::Razoavel => "razoavel",
            Forca::Forte => "forte",
            Forca::MuitoForte => "muito-forte",
        }
    }
}

/// O que `gerar_senha` devolve: nada do valor (nem um caractere).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gerada {
    pub tamanho: usize,
    /// Entropia arredondada para baixo, de 10 em 10 ("cerca de 150 bits").
    pub bits: u32,
    pub forca: Forca,
    /// Símbolos de D35, de `ward_core::generator::SYMBOLS`.
    pub simbolos: &'static str,
}

/// Símbolos de D35 como texto (são ASCII).
pub fn simbolos() -> &'static str {
    // `SIMBOLOS` é ASCII por construção; o teste confere.
    core::str::from_utf8(nucleo::SYMBOLS).unwrap_or("")
}

/// Entropia em bits de uma senha uniforme com essas opções, arredondada para baixo de
/// 10 em 10. Ignora o pequeno desconto de "um de cada conjunto" (menos de 1 bit).
fn bits(o: &GeneratorOptions) -> u32 {
    let alfabeto = [
        (o.lowercase, nucleo::LOWERCASE.len()),
        (o.uppercase, nucleo::UPPERCASE.len()),
        (o.digits, nucleo::DIGITS.len()),
        (o.symbols, nucleo::SYMBOLS.len()),
    ]
    .iter()
    .filter(|(m, _)| *m)
    .map(|(_, n)| *n)
    .sum::<usize>();
    // tamanho ≤ 64 e alfabeto ≤ 90: cabe com folga em f64 e em u32.
    let total = f64::from(u32::try_from(o.size).unwrap_or(0))
        * f64::from(u32::try_from(alfabeto).unwrap_or(0)).log2();
    let inteiro = total.floor().clamp(0.0, 10_000.0) as u32;
    inteiro - inteiro % 10
}

/// Valor revelado. Zera ao sair; o `Debug` não mostra o valor. Só o comando
/// `revelar_senha_gerada` o cria.
pub struct Revelada(Zeroizing<String>);

impl Revelada {
    /// Valor de um segredo do cofre para o "revelar" do formulário (Fase 33). Só UTF-8.
    pub(crate) fn de_segredo(s: &Segredo) -> Option<Revelada> {
        let texto = core::str::from_utf8(s.expor()).ok()?;
        Some(Revelada(Zeroizing::new(texto.to_owned())))
    }

    /// O texto, só para a serialização da resposta do "revelar".
    pub fn expor(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Revelada {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Revelada(••••)")
    }
}

/// A senha avulsa atual, só em memória do lado Rust.
#[derive(Default)]
pub struct Gerador {
    atual: Option<Segredo>,
}

impl Gerador {
    /// Gera uma senha nova (a anterior zera) e devolve só os dados sem segredo.
    pub fn gerar(&mut self, opcoes: Opcoes) -> Result<Gerada, ErroApp> {
        let o = opcoes.validar()?;
        // Antes de gerar: se o RNG falhar, não sobra a senha antiga com opções novas.
        self.atual = None;
        let senha = nucleo::generate(&o).map_err(|e| match e {
            nucleo::GeneratorError::RandomUnavailable => ErroApp::AleatorioIndisponivel,
            nucleo::GeneratorError::InvalidSize | nucleo::GeneratorError::NoSet => {
                ErroApp::PedidoInvalido
            }
        })?;
        // Copia para o tipo do app, que zera ao sair; o `ward_core::Segredo` zera no drop.
        self.atual = Some(Segredo::novo(senha.expose().to_vec()));
        drop(senha);
        let bits = bits(&o);
        Ok(Gerada {
            tamanho: o.size,
            bits,
            forca: Forca::de_bits(bits),
            simbolos: simbolos(),
        })
    }

    /// Uma cópia para a área de transferência (o `Copiador` toma posse e zera).
    pub fn para_copiar(&self) -> Result<Segredo, ErroApp> {
        self.atual
            .as_ref()
            .map(|s| Segredo::novo(s.expor().to_vec()))
            .ok_or(ErroApp::SemSenhaGerada)
    }

    /// O valor para o "revelar".
    pub fn revelar(&self) -> Result<Revelada, ErroApp> {
        let s = self.atual.as_ref().ok_or(ErroApp::SemSenhaGerada)?;
        let texto = core::str::from_utf8(s.expor()).map_err(|_| ErroApp::SemSenhaGerada)?;
        Ok(Revelada(Zeroizing::new(texto.to_owned())))
    }

    /// Zera a senha (ao sair da tela, travar ou fechar o app).
    pub fn descartar(&mut self) {
        self.atual = None;
    }

    #[cfg(test)]
    pub(crate) fn tem_senha(&self) -> bool {
        self.atual.is_some()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn opcoes(tamanho: u64) -> Opcoes {
        Opcoes {
            tamanho,
            minusculas: true,
            maiusculas: true,
            digitos: true,
            simbolos: true,
        }
    }

    #[test]
    fn limites_da_tela_dentro_do_nucleo() {
        const { assert!(TAMANHO_MIN >= nucleo::MIN_SIZE) };
        const { assert!(TAMANHO_MAX <= nucleo::MAX_SIZE) };
        const { assert!(TAMANHO_MIN <= TAMANHO_PADRAO && TAMANHO_PADRAO <= TAMANHO_MAX) };
    }

    #[test]
    fn simbolos_sao_os_de_d35() {
        assert_eq!(simbolos(), "!#$%&()*+,-./:;<=>?@[]^_{|}~");
        for proibido in [' ', '"', '\'', '`', '\\'] {
            assert!(!simbolos().contains(proibido), "{proibido:?}");
        }
    }

    #[test]
    fn forca_do_design() {
        // Design: 24 caracteres, quatro conjuntos → "muito forte · cerca de 150 bits".
        let mut g = Gerador::default();
        let r = g.gerar(opcoes(24)).unwrap();
        assert_eq!((r.tamanho, r.bits, r.forca), (24, 150, Forca::MuitoForte));
        let so_digitos = Opcoes {
            tamanho: 8,
            minusculas: false,
            maiusculas: false,
            digitos: true,
            simbolos: false,
        };
        let r = g.gerar(so_digitos).unwrap();
        assert_eq!((r.bits, r.forca), (20, Forca::Fraca));
        let r = g.gerar(opcoes(8)).unwrap();
        assert_eq!((r.bits, r.forca), (50, Forca::Fraca));
        let r = g.gerar(opcoes(12)).unwrap();
        assert_eq!((r.bits, r.forca), (70, Forca::Razoavel));
        let r = g.gerar(opcoes(16)).unwrap();
        assert_eq!((r.bits, r.forca), (100, Forca::Forte));
        let r = g.gerar(opcoes(64)).unwrap();
        assert_eq!(r.forca, Forca::MuitoForte);
    }

    #[test]
    fn senha_respeita_tamanho_e_conjuntos() {
        let mut g = Gerador::default();
        let sem_simbolos = Opcoes {
            simbolos: false,
            ..opcoes(32)
        };
        for _ in 0..50 {
            g.gerar(sem_simbolos).unwrap();
            let v = g.revelar().unwrap();
            assert_eq!(v.expor().len(), 32);
            assert!(v.expor().bytes().all(|b| b.is_ascii_alphanumeric()));
        }
        for _ in 0..50 {
            g.gerar(opcoes(8)).unwrap();
            let v = g.revelar().unwrap();
            assert!(
                v.expor()
                    .bytes()
                    .any(|b| simbolos().as_bytes().contains(&b))
            );
        }
    }

    /// Entrada da interface fora do formato: tamanho fora de 8–64 (inclusive valores que
    /// o núcleo aceitaria e um `u64` enorme) e nenhum conjunto. Nada é gerado e a senha
    /// anterior não volta a ficar disponível com opções que falharam.
    #[test]
    fn ataque_opcoes_fora_do_formato_sao_recusadas() {
        let mut g = Gerador::default();
        for t in [0, 4, 7, 65, 128, 129, u64::from(u32::MAX) + 1, u64::MAX] {
            assert_eq!(g.gerar(opcoes(t)), Err(ErroApp::PedidoInvalido), "{t}");
        }
        let nenhum = Opcoes {
            tamanho: 24,
            minusculas: false,
            maiusculas: false,
            digitos: false,
            simbolos: false,
        };
        assert_eq!(g.gerar(nenhum), Err(ErroApp::PedidoInvalido));
        assert!(!g.tem_senha());
        assert_eq!(g.revelar().err(), Some(ErroApp::SemSenhaGerada));
        assert_eq!(g.para_copiar().err(), Some(ErroApp::SemSenhaGerada));
    }

    /// A resposta do "gerar" não carrega o valor: nem no tipo nem no `Debug`.
    #[test]
    fn ataque_gerar_nao_devolve_o_valor() {
        let mut g = Gerador::default();
        let r = g.gerar(opcoes(64)).unwrap();
        let v = g.revelar().unwrap();
        let texto = format!("{r:?} {v:?}");
        assert!(!texto.contains(v.expor()));
        assert!(texto.contains("Revelada(••••)"));
        // Nenhum pedaço de 6 caracteres do valor aparece na resposta.
        for i in 0..v.expor().len() - 6 {
            assert!(!format!("{r:?}").contains(&v.expor()[i..i + 6]));
        }
    }

    #[test]
    fn descartar_zera_e_gerar_de_novo_troca() {
        let mut g = Gerador::default();
        g.gerar(opcoes(24)).unwrap();
        let a = g.revelar().unwrap().expor().to_owned();
        g.gerar(opcoes(24)).unwrap();
        let b = g.revelar().unwrap().expor().to_owned();
        assert_ne!(a, b);
        g.descartar();
        assert_eq!(g.revelar().err(), Some(ErroApp::SemSenhaGerada));
    }
}
