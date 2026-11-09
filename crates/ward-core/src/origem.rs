//! Origem de uma URL para o preenchimento (D9; Fase 40): esquema, host e porta, pelo
//! mesmo parser estrito da URL do `login` (D41, D55). Duas URLs só têm a mesma origem
//! quando os três batem; subdomínio não conta como igual, `localhost` não é
//! `127.0.0.1`, e `http` só existe para esses dois hosts (o parser recusa o resto).
//!
//! A porta padrão do esquema (443 no `https`, 80 no `http`) é igual à porta omitida:
//! o Chrome tira a porta padrão da URL da aba, e a URL da entrada pode tê-la escrita.
//! Qualquer texto que o parser recusa (outro esquema, `chrome://`, `file://`, userinfo,
//! host em maiúsculas, IP em forma hex, controle) não tem origem: não casa com nada.

use crate::entrada::validacao::{separar_host_porta, url_valida};

/// Esquema, host (em minúsculas, como o parser exige) e porta efetiva.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origem {
    https: bool,
    host: String,
    porta: u16,
}

impl Origem {
    /// Origem da URL, ou `None` se a URL não passa no parser estrito.
    pub fn da_url(url: &str) -> Option<Origem> {
        if !url_valida(url) {
            return None;
        }
        let (https, resto) = match url.strip_prefix("https://") {
            Some(r) => (true, r),
            None => (false, url.strip_prefix("http://")?),
        };
        let fim = resto.find(['/', '?', '#']).unwrap_or(resto.len());
        let (host, porta) = separar_host_porta(&resto[..fim])?;
        let porta = match porta {
            Some(p) => p.parse().ok()?,
            None if https => 443,
            None => 80,
        };
        Some(Origem {
            https,
            host: host.to_owned(),
            porta,
        })
    }
}

/// As duas URLs têm a mesma origem (D9)? Falso se qualquer uma não tiver origem.
pub fn mesma_origem(url_entrada: &str, url_aba: &str) -> bool {
    match (Origem::da_url(url_entrada), Origem::da_url(url_aba)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn mesma_origem_com_caminho_e_porta_padrao() {
        let e = "http://localhost:8081/wp-login.php";
        assert!(mesma_origem(e, "http://localhost:8081"));
        assert!(mesma_origem(e, "http://localhost:8081/wp-admin/?x=1#y"));
        assert!(mesma_origem(
            "https://app.clientex.com.br/login",
            "https://app.clientex.com.br"
        ));
        assert!(mesma_origem(
            "https://app.clientex.com.br:443/",
            "https://app.clientex.com.br/"
        ));
        assert!(mesma_origem("http://localhost/", "http://localhost:80/"));
        assert!(mesma_origem("https://[::1]:8443/", "https://[::1]:8443/x"));
    }

    #[test]
    fn ataque_origens_parecidas_nao_casam() {
        let e = "http://localhost:8081/wp-login.php";
        for aba in [
            // V6.2: outro host para o mesmo serviço.
            "http://127.0.0.1:8081/wp-login.php",
            // Outra porta (D28) e o phishing de teste.
            "http://localhost:8083/login",
            "http://localhost:8082/wp-login.php",
            "http://localhost/wp-login.php",
            // Outro esquema.
            "https://localhost:8081/wp-login.php",
            // Userinfo e disfarces: o parser recusa.
            "http://localhost:8081@evil.invalid/",
            "http://evil.invalid@localhost:8081/",
            "http://localhost%3a8081/",
            "http://LOCALHOST:8081/",
            "http://localhost:08081/",
            "http://localhost:8081\\@evil/",
            "chrome-extension://abc/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "",
            "http://localhost:8081 /",
            "http://localhost:8081\n/",
        ] {
            assert!(!mesma_origem(e, aba), "{aba:?}");
        }
        let p = "https://app.clientex.com.br/login";
        for aba in [
            "https://evil.app.clientex.com.br/login",
            "https://clientex.com.br/login",
            "https://app.clientex.com.br.evil.invalid/login",
            "https://app.clientex.com.br:8443/login",
            "http://app.clientex.com.br/login",
            "https://xn--app-clientex-com-br/",
            "https://app.clientex.com.br./login",
        ] {
            assert!(!mesma_origem(p, aba), "{aba:?}");
        }
    }

    #[test]
    fn url_invalida_nao_tem_origem() {
        assert!(Origem::da_url("http://evil.invalid/").is_none());
        assert!(Origem::da_url("https://0x7f000001/").is_none());
        assert!(!mesma_origem(
            "http://evil.invalid/",
            "http://evil.invalid/"
        ));
    }
}
