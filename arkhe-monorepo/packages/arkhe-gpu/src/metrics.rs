//! Métricas contabilísticas em linha do mock (bloco 1011, v390.2).
//!
//! Contadores atômicos — os mesmos agregados que o núcleo Lean I537-A/B prova
//! (violações ≤ lançamentos; bem-formada ⟹ zero violações), aqui em hardware
//! de contagem partilhado pelo mock.

use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// Contadores agregados de uma instância de backend.
#[derive(Debug, Default)]
pub struct Metrics {
    launches: AtomicUsize,
    failures: AtomicUsize,
    violations_tracked: AtomicU64,
    busy: AtomicBool,
}

impl Metrics {
    /// Novos contadores zerados.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Regista um lance executado.
    pub fn observe_launch(&self) {
        self.launches.fetch_add(1, Ordering::Relaxed);
    }

    /// Regista uma falha de execução.
    pub fn observe_failure(&self) {
        self.failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Regista uma violação de contrato (por contabilidade — o mock nunca a
    /// produz por `launch`; só testes sintéticos a contam).
    pub fn observe_violation(&self) {
        self.violations_tracked.fetch_add(1, Ordering::Relaxed);
    }

    /// N.º de lançamentos observados.
    #[must_use]
    pub fn launches(&self) -> usize {
        self.launches.load(Ordering::Relaxed)
    }

    /// N.º de falhas observadas.
    #[must_use]
    pub fn failures(&self) -> usize {
        self.failures.load(Ordering::Relaxed)
    }

    /// N.º de violações de contrato contadas.
    #[must_use]
    pub fn violations(&self) -> u64 {
        self.violations_tracked.load(Ordering::Relaxed)
    }

    /// Lança o flag `busy` (sincronização em curso); `false` se já ocupado.
    #[must_use]
    pub fn try_acquire_busy(&self) -> bool {
        !self.busy.swap(true, Ordering::AcqRel)
    }

    /// Libera o flag `busy`.
    pub fn release_busy(&self) {
        self.busy.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_counters() {
        let m = Metrics::new();
        m.observe_launch();
        m.observe_launch();
        m.observe_failure();
        assert_eq!(m.launches(), 2);
        assert_eq!(m.failures(), 1);
        assert!(m.try_acquire_busy());
        assert!(!m.try_acquire_busy(), "já ocupado");
        m.release_busy();
        assert!(m.try_acquire_busy());
    }
}