//! Origin of a URL for autofill (D9; Phase 40): scheme, host and port, by the same
//! strict URL parser of `login` (D41, D55). Two URLs only have the same origin when the
//! three match; a subdomain does not count as equal, `localhost` is not `127.0.0.1`, and
//! `http` only exists for those two hosts (the parser refuses the rest).
//!
//! The default port of the scheme (443 for `https`, 80 for `http`) equals the omitted
//! port: Chrome strips the default port from the tab URL, and the entry URL may have it
//! written. Any text the parser refuses (another scheme, `chrome://`, `file://`,
//! userinfo, uppercase host, IP in hex form, control) has no origin: it matches nothing.

use crate::entry::validation::{split_host_port, valid_url};

/// Scheme, host (lowercase, as the parser requires) and effective port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    https: bool,
    host: String,
    port: u16,
}

impl Origin {
    /// Origin of the URL, or `None` if the URL does not pass the strict parser.
    pub fn from_url(url: &str) -> Option<Origin> {
        if !valid_url(url) {
            return None;
        }
        let (https, rest) = match url.strip_prefix("https://") {
            Some(r) => (true, r),
            None => (false, url.strip_prefix("http://")?),
        };
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let (host, port) = split_host_port(&rest[..end])?;
        let port = match port {
            Some(p) => p.parse().ok()?,
            None if https => 443,
            None => 80,
        };
        Some(Origin {
            https,
            host: host.to_owned(),
            port,
        })
    }
}

/// Do the two URLs have the same origin (D9)? False if either one has no origin.
pub fn same_origin(entry_url: &str, tab_url: &str) -> bool {
    match (Origin::from_url(entry_url), Origin::from_url(tab_url)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_origin_with_path_and_default_port() {
        let e = "http://localhost:8081/wp-login.php";
        assert!(same_origin(e, "http://localhost:8081"));
        assert!(same_origin(e, "http://localhost:8081/wp-admin/?x=1#y"));
        assert!(same_origin(
            "https://app.clientex.com.br/login",
            "https://app.clientex.com.br"
        ));
        assert!(same_origin(
            "https://app.clientex.com.br:443/",
            "https://app.clientex.com.br/"
        ));
        assert!(same_origin("http://localhost/", "http://localhost:80/"));
        assert!(same_origin("https://[::1]:8443/", "https://[::1]:8443/x"));
    }

    #[test]
    fn attack_lookalike_origins_do_not_match() {
        let e = "http://localhost:8081/wp-login.php";
        for tab in [
            // V6.2: another host for the same service.
            "http://127.0.0.1:8081/wp-login.php",
            // Another port (D28) and the test phishing.
            "http://localhost:8083/login",
            "http://localhost:8082/wp-login.php",
            "http://localhost/wp-login.php",
            // Another scheme.
            "https://localhost:8081/wp-login.php",
            // Userinfo and disguises: the parser refuses.
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
            assert!(!same_origin(e, tab), "{tab:?}");
        }
        let p = "https://app.clientex.com.br/login";
        for tab in [
            "https://evil.app.clientex.com.br/login",
            "https://clientex.com.br/login",
            "https://app.clientex.com.br.evil.invalid/login",
            "https://app.clientex.com.br:8443/login",
            "http://app.clientex.com.br/login",
            "https://xn--app-clientex-com-br/",
            "https://app.clientex.com.br./login",
        ] {
            assert!(!same_origin(p, tab), "{tab:?}");
        }
    }

    #[test]
    fn invalid_url_has_no_origin() {
        assert!(Origin::from_url("http://evil.invalid/").is_none());
        assert!(Origin::from_url("https://0x7f000001/").is_none());
        assert!(!same_origin("http://evil.invalid/", "http://evil.invalid/"));
    }
}
