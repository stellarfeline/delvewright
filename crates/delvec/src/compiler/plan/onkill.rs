//! On-kill: whether a fight comes back after the party has met it.

use super::*;

impl<'a> Plan<'a> {
    /// Does this fight come back after the party has met it (spec-0074 §4) — a
    /// rest re-seats it, or the beat that seats it can fire more than once? The
    /// one derivation ([`crate::compiler::onkill::fight_comes_back`]), asked with
    /// this plan's collected rest points.
    pub fn fight_comes_back(
        &self,
        fight: delvewright_dsl::Fight<'_>,
    ) -> Option<crate::compiler::onkill::ComesBack> {
        crate::compiler::onkill::fight_comes_back(
            self.campaign,
            self.bonfires().next().is_some(),
            fight,
        )
    }
}
