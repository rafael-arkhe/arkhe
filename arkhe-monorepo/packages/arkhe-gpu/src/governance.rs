//! Governança de lançamentos da camada GPU (bloco 1011, v390.2) — invariantes
//! **I536/I537/I538**.
//!
//! Espelho Rust do núcleo `src/lean/GpuInvariantsNucleus.lean` (core Lean
//! 4.33.1, sem Mathlib, sem `sorry`): as constantes, a banda de geometria
//! (G-07) e as capas temporais (G-07/G-08) são os MESMOS valores que o kernel
//! elaborou — a paridade fecha por teste FFI contra o texto.

use crate::audit::{AuditEntry, LaunchStatus};

/// Cap máx. de threads por bloco (CUDA: 1024) — G-07.
pub const MAX_THREADS_PER_BLOCK: u64 = 1024;

/// Cap máx. de blocos na grelha (2³¹−1) — G-07.
pub const MAX_GRID: u64 = 2_147_483_647;

/// Capa de duração da track Tile (5 s) — resposta `gpu_rules.pl`.
pub const TILE_CAP_MS: u64 = 5_000;

/// Capa de duração da track SIMT (10 s) — resposta `gpu_rules.pl`.
pub const SIMT_CAP_MS: u64 = 10_000;

/// Geometria de lançamento autorizada (G-07): threads e grelha dentro da banda
/// constitucional. Espelho de `allowed_geometry` no núcleo Lean.
#[must_use]
pub fn allowed_geometry(threads: u64, grid: u64) -> bool {
    0 < threads && threads <= MAX_THREADS_PER_BLOCK && 0 < grid && grid <= MAX_GRID
}

/// Duração aceite na track Tile (G-07): ≤ 5 s.
#[must_use]
pub fn tile_duration_ok(ms: u64) -> bool {
    ms <= TILE_CAP_MS
}

/// Duração aceite na track SIMT (G-07): ≤ 10 s.
#[must_use]
pub fn simt_duration_ok(ms: u64) -> bool {
    ms <= SIMT_CAP_MS
}

/// Causa de recusa tipada da governança (espelho de I536 em Rust).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// `threads` ou `grid` fora da banda inteira de geometria G-07.
    GeometryOutOfBand,
    /// Duração acima da capa da track (Tile 5 s / SIMT 10 s).
    DurationBeyondCap,
}

/// Decisão da governança para um lance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchDecision {
    /// Lancamento autorizado: geometria na banda G-07 e duração na capa da
    /// track.
    Allow,
    /// Lancamento recusado, com a causa honesta.
    Reject(RejectReason),
}

impl LaunchDecision {
    /// `true` quando o lance é autorizado.
    #[must_use]
    pub fn allowed(self) -> bool {
        matches!(self, Self::Allow)
    }

    /// Causa da recusa (ou `None` quando autorizado).
    #[must_use]
    pub fn reject_reason(self) -> Option<RejectReason> {
        match self {
            Self::Allow => None,
            Self::Reject(r) => Some(r),
        }
    }
}

/// Decide um lance: geometria na banda G-07 **e** duração na capa da track.
#[must_use]
pub fn evaluate(threads: u64, grid: u64, duration_ms: u64, track_cap_ms: u64) -> LaunchDecision {
    if !allowed_geometry(threads, grid) {
        return LaunchDecision::Reject(RejectReason::GeometryOutOfBand);
    }
    if duration_ms > track_cap_ms {
        return LaunchDecision::Reject(RejectReason::DurationBeyondCap);
    }
    LaunchDecision::Allow
}

/// I536-A: numa trilha bem-formada, todo lançamento registado satisfez o
/// contrato (a auditoria não pode «esquecer» a violação que a governança
/// teria recusado). Implicação material — idem núcleo Lean.
#[must_use]
pub fn i536a_wellformed_entries_satisfied(entries: &[AuditEntry]) -> bool {
    !crate::audit::is_well_formed(entries)
        || entries
            .iter()
            .all(|e| e.status == LaunchStatus::Satisfied)
}

/// I536-B: uma geometria autorizada nunca excede o cap de threads por bloco.
#[must_use]
pub fn i536b_threads_within_band(threads: u64, grid: u64) -> bool {
    !allowed_geometry(threads, grid) || threads <= MAX_THREADS_PER_BLOCK
}

/// I536-C: uma geometria autorizada nunca excede o cap de blocos da grelha.
#[must_use]
pub fn i536c_grid_within_band(threads: u64, grid: u64) -> bool {
    !allowed_geometry(threads, grid) || grid <= MAX_GRID
}

/// I536-D: uma geometria autorizada é estritamente positiva nas duas
/// dimensões.
#[must_use]
pub fn i536d_geometry_positive(threads: u64, grid: u64) -> bool {
    !allowed_geometry(threads, grid) || (0 < threads && 0 < grid)
}

/// I537-A: o n.º de violações de contrato registadas nunca excede o n.º de
/// lançamentos registados.
#[must_use]
pub fn i537a_violations_le_launches(entries: &[AuditEntry]) -> bool {
    crate::audit::contract_violations(entries) <= crate::audit::launches(entries)
}

/// I537-B: numa trilha bem-formada, não há violações de contrato registadas.
#[must_use]
pub fn i537b_wellformed_has_no_violations(entries: &[AuditEntry]) -> bool {
    !crate::audit::is_well_formed(entries) || crate::audit::contract_violations(entries) == 0
}

/// I538-A: uma duração aceite na capa Tile (≤ 5 s), composta com uma retenção/
/// merge `k ≤ 5 s`, permanece dentro da capa SIMT (≤ 10 s) — fecho da banda sob
/// retenção aditiva. Implicação material — idem núcleo Lean.
#[must_use]
pub fn i538a_temporal_band_closed_under_retention(ms: u64, k: u64) -> bool {
    !tile_duration_ok(ms) || k > TILE_CAP_MS || simt_duration_ok(ms + k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_band_exact_edges() {
        assert!(!allowed_geometry(0, 1));
        assert!(allowed_geometry(1, 1));
        assert!(allowed_geometry(MAX_THREADS_PER_BLOCK, MAX_GRID));
        assert!(!allowed_geometry(MAX_THREADS_PER_BLOCK + 1, 1));
        assert!(!allowed_geometry(1, MAX_GRID + 1));
        assert!(!allowed_geometry(1, 0));
    }

    #[test]
    fn i536b_c_threads_and_grid_never_above_band() {
        for (t, g) in [(0, 0), (1, 1), (1024, 2_147_483_647), (1025, 1), (1, 2_147_483_648)] {
            assert!(i536b_threads_within_band(t, g), "threads banda {t},{g}");
            assert!(i536c_grid_within_band(t, g), "grid banda {t},{g}");
        }
        assert!(!i536d_geometry_positive(0, 1) || !allowed_geometry(0, 1));
        assert!(i536d_geometry_positive(1, 1));
    }

    #[test]
    fn track_caps_and_i538_retention_closure() {
        assert!(tile_duration_ok(5_000));
        assert!(!tile_duration_ok(5_001));
        assert!(simt_duration_ok(10_000));
        assert!(!simt_duration_ok(10_001));
        for ms in [0, 1, 2_500, 5_000] {
            for k in [0, 1, 5_000] {
                assert!(i538a_temporal_band_closed_under_retention(ms, k), "ms={ms},k={k}");
            }
        }
        assert!(i538a_temporal_band_closed_under_retention(5_000, 5_000));
    }

    #[test]
    fn evaluate_decision_typed() {
        assert_eq!(
            evaluate(0, 1, 100, TILE_CAP_MS),
            LaunchDecision::Reject(RejectReason::GeometryOutOfBand)
        );
        assert_eq!(
            evaluate(32, 1, 5_001, TILE_CAP_MS),
            LaunchDecision::Reject(RejectReason::DurationBeyondCap)
        );
        assert_eq!(evaluate(32, 1, 5_000, TILE_CAP_MS), LaunchDecision::Allow);
    }
}