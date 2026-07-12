//! FI-W19 — Origem verificada de conexões de wallet (wallet connection phishing, DNS hijacking).
//!
//! `∀ connection, verified_origin(connection)`

use crate::InvariantVerdict;

/// FI-W19: a origem de uma conexão de carteira deve estar na allowlist de
/// domínios confiáveis e servida sobre HTTPS.
pub fn check_verified_origin(origin: &str, allowed_domains: &[&str]) -> InvariantVerdict {
    if !origin.starts_with("https://") {
        return InvariantVerdict::violated(format!("origin '{origin}' is not HTTPS"));
    }
    let host = origin.trim_start_matches("https://");
    if allowed_domains.iter().any(|d| host == *d || host.starts_with(&format!("{d}/"))) {
        InvariantVerdict::Holds
    } else {
        InvariantVerdict::violated(format!("origin '{origin}' is not in allowed domain list"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_https_origin_holds() {
        let verdict = check_verified_origin("https://app.arkhe.io", &["app.arkhe.io"]);
        assert!(verdict.holds());
    }

    #[test]
    fn non_https_origin_is_rejected() {
        let verdict = check_verified_origin("http://app.arkhe.io", &["app.arkhe.io"]);
        assert!(!verdict.holds());
    }

    #[test]
    fn phishing_lookalike_domain_is_rejected() {
        // DNS hijacking / lookalike domain not in allowlist.
        let verdict = check_verified_origin("https://app-arkhe.io.evil.com", &["app.arkhe.io"]);
        assert!(!verdict.holds());
    }
}
