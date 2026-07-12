//! FI-W18 — Rate limiting e autenticação de APIs/indexers/relayers off-chain.
//!
//! `∀ request, authenticated(request) ∧ rate_limited(request)`

use crate::InvariantVerdict;
use std::collections::HashMap;

/// Requisição recebida por uma API off-chain (relayer, indexer, etc).
#[derive(Debug, Clone)]
pub struct ApiRequest {
    pub caller: String,
    pub authenticated: bool,
}

/// Janela deslizante simples por chamador, reaproveitando o mesmo formato de
/// `arkhe-rate-limit` mas escopado a requisições Web3 (RPC, indexers).
#[derive(Debug, Default)]
pub struct ApiGate {
    max_requests_per_window: usize,
    counts: HashMap<String, usize>,
}

impl ApiGate {
    pub fn new(max_requests_per_window: usize) -> Self {
        Self { max_requests_per_window, counts: HashMap::new() }
    }

    /// FI-W18: rejeita requisições não autenticadas ou que excedam o limite.
    pub fn check_request(&mut self, req: &ApiRequest) -> InvariantVerdict {
        if !req.authenticated {
            return InvariantVerdict::violated(format!(
                "request from '{}' is not authenticated",
                req.caller
            ));
        }
        let count = self.counts.entry(req.caller.clone()).or_insert(0);
        *count += 1;
        if *count > self.max_requests_per_window {
            return InvariantVerdict::violated(format!(
                "caller '{}' exceeded rate limit ({} > {})",
                req.caller, count, self.max_requests_per_window
            ));
        }
        InvariantVerdict::Holds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticated_request_within_limit_holds() {
        let mut gate = ApiGate::new(2);
        let req = ApiRequest { caller: "alice".into(), authenticated: true };
        assert!(gate.check_request(&req).holds());
        assert!(gate.check_request(&req).holds());
    }

    #[test]
    fn unauthenticated_request_is_rejected() {
        let mut gate = ApiGate::new(2);
        let req = ApiRequest { caller: "alice".into(), authenticated: false };
        assert!(!gate.check_request(&req).holds());
    }

    #[test]
    fn request_over_limit_is_rejected() {
        let mut gate = ApiGate::new(1);
        let req = ApiRequest { caller: "alice".into(), authenticated: true };
        assert!(gate.check_request(&req).holds());
        assert!(!gate.check_request(&req).holds());
    }
}
