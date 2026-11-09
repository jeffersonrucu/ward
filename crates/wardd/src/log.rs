//! Log do daemon em stderr (o journal, pela unit; `.dev/logs/` no daemon de teste).
//!
//! S13: as mensagens são texto fixo mais números, nomes de operação, papel e códigos de
//! erro. Nenhuma chamada recebe valor de pedido, de entrada, de caminho do cliente nem
//! de variável de ambiente; quem chama passa só `&'static str` e inteiros. A exceção é
//! o `revelar`/`copiar`/`totp` do app (D65 b), que registra o nome da entrada (do
//! cofre, já validado) e o campo (da lista fixa do protocolo, ou o nome do extra com
//! escape), nunca o valor. Com
//! `RUST_LOG` em `debug` ou `trace`, sai também uma linha por pedido (operação, papel e
//! resultado), nunca o conteúdo.

use std::sync::OnceLock;

static DETALHADO: OnceLock<bool> = OnceLock::new();

/// Lê `RUST_LOG` uma vez (só o nível; o valor nunca é impresso).
pub fn configurar() {
    let detalhado = std::env::var_os("RUST_LOG").is_some_and(|v| {
        let v = v.as_encoded_bytes();
        [b"debug".as_slice(), b"trace".as_slice()]
            .iter()
            .any(|n| v.windows(n.len()).any(|w| w == *n))
    });
    let _ = DETALHADO.set(detalhado);
}

/// Linha de log sempre impressa.
#[macro_export]
macro_rules! aviso {
    ($($arg:tt)*) => {
        eprintln!("wardd: {}", format_args!($($arg)*))
    };
}

/// Linha só com `RUST_LOG=debug|trace`.
#[macro_export]
macro_rules! depurar {
    ($($arg:tt)*) => {
        if $crate::log::detalhado() {
            eprintln!("wardd[debug]: {}", format_args!($($arg)*))
        }
    };
}

pub fn detalhado() -> bool {
    DETALHADO.get().copied().unwrap_or(false)
}
