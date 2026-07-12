//! FI-032 — evaluating an `arkhe_reasoning::Plan` against a live
//! [`Lifecycle`]: `PlanKind::Execution` plans must finish before their
//! deadline (FI-055); `PlanKind::Service` plans have no deadline and are
//! monitored via health checks (`arkhe-health-check`) instead. This module
//! is deliberately thin — it composes FI-031 (acyclic validation), FI-055
//! (deadlines), and `arkhe-health-check`'s existing `HealthChecker` rather
//! than inventing new validation or health machinery.

use arkhe_health_check::{HealthChecker, HealthReport};
use arkhe_reasoning::{ActionId, Plan, PlanError, PlanKind, PlanValidator};

use crate::lifecycle::Lifecycle;

/// The result of evaluating a plan once against a `Lifecycle`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanOutcome {
    /// `Execution` plan: validated, and its `Lifecycle` had not passed its
    /// deadline. A `Lifecycle` with no deadline set never times out (see
    /// [`Lifecycle::is_past_deadline`]), so an execution plan on such a VM
    /// is `Done` as soon as it validates.
    Done,
    /// The plan's dependency graph is invalid, or (for `Execution` plans
    /// only) the `Lifecycle`'s deadline had already passed.
    Failed(PlanFailure),
    /// `Service` plan: validated and currently alive. Not a terminal
    /// outcome — service plans don't "finish"; see
    /// [`evaluate_service_health`] for their ongoing health signal.
    Running,
}

/// Why a plan evaluation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanFailure {
    /// The plan's action-dependency graph has a cycle (FI-031).
    Cyclic(Vec<ActionId>),
    /// An action depends on an action ID that isn't in the plan (FI-031).
    UnknownDependency { action: ActionId, missing_dependency: ActionId },
    /// `Execution` plan whose `Lifecycle` deadline had already passed
    /// (FI-055) at evaluation time.
    DeadlineExceeded,
}

impl From<PlanError> for PlanFailure {
    fn from(err: PlanError) -> Self {
        match err {
            PlanError::Cyclic(ids) => PlanFailure::Cyclic(ids),
            PlanError::UnknownDependency { action, missing_dependency } => {
                PlanFailure::UnknownDependency { action, missing_dependency }
            }
        }
    }
}

/// Evaluates `plan` of the given `kind` against `lifecycle`'s current
/// state. Validates the plan is acyclic first (FI-031) — an invalid plan
/// fails regardless of kind, before any deadline check runs.
pub fn evaluate_plan(plan: &Plan, kind: PlanKind, lifecycle: &Lifecycle) -> PlanOutcome {
    if let Err(err) = PlanValidator::new().validate(plan) {
        return PlanOutcome::Failed(err.into());
    }
    match kind {
        PlanKind::Execution => {
            if lifecycle.is_past_deadline() {
                PlanOutcome::Failed(PlanFailure::DeadlineExceeded)
            } else {
                PlanOutcome::Done
            }
        }
        PlanKind::Service => PlanOutcome::Running,
    }
}

/// Aggregates health for a `PlanKind::Service` plan's registered checks.
/// A thin wrapper over `arkhe_health_check::HealthChecker::readiness` — no
/// new health-check machinery introduced here.
pub fn evaluate_service_health(checker: &HealthChecker) -> HealthReport {
    checker.readiness()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_health_check::{healthy, unhealthy, HealthCheckable, HealthStatus};
    use arkhe_reasoning::Action;

    fn linear_plan() -> Plan {
        Plan { actions: vec![Action { id: 1, name: "a".into(), depends_on: vec![] }] }
    }

    fn cyclic_plan() -> Plan {
        Plan { actions: vec![Action { id: 1, name: "a".into(), depends_on: vec![1] }] }
    }

    #[test]
    fn execution_plan_past_its_deadline_is_failed() {
        let mut lc = Lifecycle::new();
        lc.set_deadline(Some(0)); // 1970 — always in the past
        let outcome = evaluate_plan(&linear_plan(), PlanKind::Execution, &lc);
        assert_eq!(outcome, PlanOutcome::Failed(PlanFailure::DeadlineExceeded));
    }

    #[test]
    fn execution_plan_with_no_deadline_is_done() {
        let lc = Lifecycle::new();
        assert_eq!(lc.deadline(), None);
        let outcome = evaluate_plan(&linear_plan(), PlanKind::Execution, &lc);
        assert_eq!(outcome, PlanOutcome::Done);
    }

    #[test]
    fn execution_plan_with_a_future_deadline_is_done() {
        let mut lc = Lifecycle::new();
        lc.set_deadline(Some(crate::lifecycle::now_secs() + 3600));
        let outcome = evaluate_plan(&linear_plan(), PlanKind::Execution, &lc);
        assert_eq!(outcome, PlanOutcome::Done);
    }

    #[test]
    fn cyclic_plan_fails_regardless_of_kind() {
        let lc = Lifecycle::new();
        assert!(matches!(
            evaluate_plan(&cyclic_plan(), PlanKind::Execution, &lc),
            PlanOutcome::Failed(PlanFailure::Cyclic(_))
        ));
        assert!(matches!(
            evaluate_plan(&cyclic_plan(), PlanKind::Service, &lc),
            PlanOutcome::Failed(PlanFailure::Cyclic(_))
        ));
    }

    #[test]
    fn service_plan_is_running_once_validated_even_past_a_deadline() {
        // A Service plan ignores the deadline entirely — deadlines are an
        // Execution-only concept; a Service plan's Lifecycle having one
        // set at all would be unusual, but the evaluation must not treat
        // it as a failure condition either way.
        let mut lc = Lifecycle::new();
        lc.set_deadline(Some(0));
        let outcome = evaluate_plan(&linear_plan(), PlanKind::Service, &lc);
        assert_eq!(outcome, PlanOutcome::Running);
    }

    struct AlwaysHealthy;
    impl HealthCheckable for AlwaysHealthy {
        fn name(&self) -> &str {
            "svc"
        }
        fn check_liveness(&self) -> arkhe_health_check::CheckResult {
            healthy("svc", "ok")
        }
        fn check_readiness(&self) -> arkhe_health_check::CheckResult {
            healthy("svc", "ok")
        }
    }

    struct AlwaysUnhealthy;
    impl HealthCheckable for AlwaysUnhealthy {
        fn name(&self) -> &str {
            "svc"
        }
        fn check_liveness(&self) -> arkhe_health_check::CheckResult {
            unhealthy("svc", "down")
        }
        fn check_readiness(&self) -> arkhe_health_check::CheckResult {
            unhealthy("svc", "down")
        }
    }

    #[test]
    fn service_health_reflects_registered_checks() {
        let mut checker = HealthChecker::new("0.1.0");
        checker.register(Box::new(AlwaysHealthy));
        assert_eq!(evaluate_service_health(&checker).status, HealthStatus::Healthy);

        let mut checker = HealthChecker::new("0.1.0");
        checker.register(Box::new(AlwaysUnhealthy));
        assert_eq!(evaluate_service_health(&checker).status, HealthStatus::Unhealthy);
    }
}
