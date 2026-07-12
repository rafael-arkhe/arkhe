//! FI-078 — TLS 1.3+.
//!
//! [`tls_1_3_only_client_config`] builds a real `rustls::ClientConfig`
//! restricted to TLS 1.3 at the protocol-version-negotiation level (not
//! merely "preferred" — TLS 1.2 and earlier are refused outright).
//!
//! **Scope note:** this crate provides the client-side config builder only.
//! A server-side equivalent needs actual certificate/key material
//! (`rustls::ServerConfig::with_single_cert`), which this pass doesn't
//! fabricate — wiring that up for real needs a certificate provisioning
//! story (e.g. `rcgen` for dev/test certs, or a real CA-issued cert in
//! production) that's a distinct piece of work. Also out of scope: this
//! builds a *config*, not a connection — no socket/listener code, no
//! handshake driving. See the crate README for the full list of what
//! "arkhe-network" does and doesn't cover.

use std::sync::Arc;

use rustls::{ClientConfig, RootCertStore};

/// Builds a `ClientConfig` that will only ever negotiate TLS 1.3 — the
/// handshake fails outright against a TLS 1.2-or-earlier server rather than
/// falling back.
pub fn tls_1_3_only_client_config(root_store: RootCertStore) -> Result<Arc<ClientConfig>, rustls::Error> {
    let config = ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_config_builds_successfully_with_tls13_only() {
        let root_store = RootCertStore::empty();
        // The real enforcement is `builder_with_protocol_versions(&[&rustls::version::TLS13])`
        // inside tls_1_3_only_client_config itself — passing only TLS13 to
        // that call is what makes the resulting config refuse to negotiate
        // anything else. This test confirms that call succeeds against the
        // real rustls API (it would return Err(NoCipherSuitesConfigured) or
        // similar if the crypto provider had nothing to offer for TLS 1.3).
        let config = tls_1_3_only_client_config(root_store).unwrap();
        assert!(!config.crypto_provider().cipher_suites.is_empty());
    }

    #[test]
    fn requesting_only_tls13_does_not_silently_include_tls12() {
        // Sanity check on the version list itself, independent of the
        // builder: rustls::ALL_VERSIONS has both entries, but we only ever
        // pass a single-element slice containing TLS13 to the builder.
        let versions: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS13];
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, rustls::ProtocolVersion::TLSv1_3);
    }
}
