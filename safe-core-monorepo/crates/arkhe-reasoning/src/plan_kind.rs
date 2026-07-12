//! FI-032 — a plan is either bounded (must terminate) or persistent
//! (expected to run indefinitely, monitored instead of timed out). This
//! distinction is what makes FI-055 (execution deadlines) and health
//! checking two different, non-overlapping enforcement mechanisms rather
//! than one mechanism half-applied to both kinds of plan.
//!
//! This type deliberately carries no reference to `arkhe-agent-vm::Lifecycle`
//! or `arkhe-health-check` — `arkhe-reasoning` stays a pure, dependency-free
//! validation crate (see the crate's own README). The actual outcome
//! evaluation against a live `Lifecycle` lives in `arkhe-agent-vm`, which
//! already depends on this crate, not the other way around.

/// What kind of plan this is, and therefore what "success" means for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanKind {
    /// Must terminate. Bounded by a deadline (FI-055) — exceeding it is a
    /// failure, not merely a delay.
    Execution,
    /// Persistent. No deadline is meaningful; health is checked
    /// periodically instead (see `arkhe-health-check`).
    Service,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_kind_is_a_plain_copy_enum() {
        let kind = PlanKind::Execution;
        let copied = kind;
        assert_eq!(kind, copied);
        assert_ne!(PlanKind::Execution, PlanKind::Service);
    }
}
