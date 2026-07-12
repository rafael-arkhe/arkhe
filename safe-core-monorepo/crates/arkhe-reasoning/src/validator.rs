//! FI-031 — acyclic plans: a plan whose action dependency graph contains a
//! cycle is rejected before execution, via Kahn's algorithm (a plan is
//! acyclic iff Kahn's algorithm can process every action).
//!
//! **Note on a bug in an earlier draft of this file:** the original sketch
//! had `plan.dependencies.get(&id).unwrap_or(&vec![])`, which does not
//! compile — `&vec![]` borrows a temporary `Vec` that's dropped at the end
//! of the statement ("temporary value dropped while borrowed" /
//! `E0716`-class error). Fixed here by keying the dependents map by
//! reference into data that's actually owned by the function (`dependents:
//! HashMap<ActionId, Vec<ActionId>>`, built once, borrowed via `.get()`
//! against its own storage — no temporary involved).

use std::collections::{HashMap, HashSet, VecDeque};

pub type ActionId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub id: ActionId,
    pub name: String,
    /// Actions that must complete before this one — matches the RHS of
    /// "A depende de B" in the catalog: `A.depends_on` contains `B`.
    pub depends_on: Vec<ActionId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum PlanError {
    /// The action IDs that could **not** be topologically sorted — every
    /// action left over once Kahn's algorithm runs out of zero-in-degree
    /// nodes to process is part of (or depends transitively on) a cycle.
    #[error("plan contains a cycle involving action(s): {0:?}")]
    Cyclic(Vec<ActionId>),
    #[error("action {action} depends on unknown action {missing_dependency}")]
    UnknownDependency { action: ActionId, missing_dependency: ActionId },
}

#[derive(Debug, Default)]
pub struct PlanValidator;

impl PlanValidator {
    pub fn new() -> Self {
        Self
    }

    /// Returns a valid execution order (topologically sorted) if `plan` is
    /// acyclic, `Err` otherwise. Does not execute anything — this is a
    /// pure validation/ordering function; a caller wires evidence recording
    /// and actual execution around it (kept separate so this stays testable
    /// without an `EvidenceBus`/async runtime).
    pub fn validate(&self, plan: &Plan) -> Result<Vec<ActionId>, PlanError> {
        let known: HashSet<ActionId> = plan.actions.iter().map(|a| a.id).collect();

        let mut in_degree: HashMap<ActionId, usize> = HashMap::new();
        let mut dependents: HashMap<ActionId, Vec<ActionId>> = HashMap::new();

        for action in &plan.actions {
            in_degree.entry(action.id).or_insert(0);
            for &dep in &action.depends_on {
                if !known.contains(&dep) {
                    return Err(PlanError::UnknownDependency { action: action.id, missing_dependency: dep });
                }
                *in_degree.entry(action.id).or_insert(0) += 1;
                dependents.entry(dep).or_default().push(action.id);
            }
        }

        let mut queue: VecDeque<ActionId> =
            in_degree.iter().filter(|(_, &deg)| deg == 0).map(|(&id, _)| id).collect();

        let mut sorted = Vec::with_capacity(plan.actions.len());
        while let Some(id) = queue.pop_front() {
            sorted.push(id);
            if let Some(deps) = dependents.get(&id) {
                for &dependent in deps {
                    let entry = in_degree.get_mut(&dependent).expect("dependent was seen while building in_degree");
                    *entry -= 1;
                    if *entry == 0 {
                        queue.push_back(dependent);
                    }
                }
            }
        }

        if sorted.len() != plan.actions.len() {
            let sorted_set: HashSet<ActionId> = sorted.iter().copied().collect();
            let remaining: Vec<ActionId> = plan.actions.iter().map(|a| a.id).filter(|id| !sorted_set.contains(id)).collect();
            return Err(PlanError::Cyclic(remaining));
        }

        Ok(sorted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(id: ActionId, depends_on: &[ActionId]) -> Action {
        Action { id, name: format!("action-{id}"), depends_on: depends_on.to_vec() }
    }

    #[test]
    fn linear_chain_is_accepted_in_dependency_order() {
        // A -> B -> C (B depends on A, C depends on B)
        let plan = Plan { actions: vec![action(1, &[]), action(2, &[1]), action(3, &[2])] };
        let order = PlanValidator::new().validate(&plan).unwrap();
        let pos = |id: ActionId| order.iter().position(|&x| x == id).unwrap();
        assert!(pos(1) < pos(2));
        assert!(pos(2) < pos(3));
    }

    #[test]
    fn direct_cycle_is_rejected() {
        // A depends on B, B depends on A.
        let plan = Plan { actions: vec![action(1, &[2]), action(2, &[1])] };
        let result = PlanValidator::new().validate(&plan);
        assert!(matches!(result, Err(PlanError::Cyclic(_))));
    }

    #[test]
    fn longer_cycle_is_rejected() {
        // A -> B -> C -> A
        let plan = Plan { actions: vec![action(1, &[3]), action(2, &[1]), action(3, &[2])] };
        let result = PlanValidator::new().validate(&plan);
        assert!(matches!(result, Err(PlanError::Cyclic(_))));
    }

    #[test]
    fn diamond_dependency_graph_is_accepted() {
        //     A
        //    / \
        //   B   C
        //    \ /
        //     D
        let plan =
            Plan { actions: vec![action(1, &[]), action(2, &[1]), action(3, &[1]), action(4, &[2, 3])] };
        let order = PlanValidator::new().validate(&plan).unwrap();
        assert_eq!(order.len(), 4);
        let pos = |id: ActionId| order.iter().position(|&x| x == id).unwrap();
        assert!(pos(1) < pos(2));
        assert!(pos(1) < pos(3));
        assert!(pos(2) < pos(4));
        assert!(pos(3) < pos(4));
    }

    #[test]
    fn empty_plan_is_accepted() {
        let plan = Plan::default();
        assert_eq!(PlanValidator::new().validate(&plan).unwrap(), Vec::<ActionId>::new());
    }

    #[test]
    fn dependency_on_unknown_action_is_rejected() {
        let plan = Plan { actions: vec![action(1, &[999])] };
        let result = PlanValidator::new().validate(&plan);
        assert_eq!(result, Err(PlanError::UnknownDependency { action: 1, missing_dependency: 999 }));
    }

    #[test]
    fn self_dependency_is_a_cycle() {
        let plan = Plan { actions: vec![action(1, &[1])] };
        let result = PlanValidator::new().validate(&plan);
        assert!(matches!(result, Err(PlanError::Cyclic(_))));
    }
}
