//! The seals a `close-gate` makes: the plan record a gate's state is read from.

use super::*;

/// The seal plan for a gate anchor, if the campaign ever seals it.
pub(super) fn seal_hint_for<'a>(plan: &'a Plan, anchor: &str) -> Option<&'a plan::SealHintPlan> {
    plan.seal_hints.iter().find(|s| s.anchor == anchor)
}
