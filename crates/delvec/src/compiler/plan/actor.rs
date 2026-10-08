//! Actors: which declared actors a rest re-seats.

use super::*;

impl<'a> Plan<'a> {
    /// The actors a bonfire refreshes while they are undefeated (spec-0016 §1),
    /// in declaration order: every actor the campaign
    /// `unleash-actor`s — the compiler's one definition of an actor that is a
    /// *fight* ([`crate::compiler::combat::hostile_actors`]).
    ///
    /// Empty without a bonfire, and empty for every campaign whose actors are all
    /// scenery → byte-identical emission.
    pub fn reseat_actors(&self) -> Vec<&delvewright_dsl::Actor> {
        if !self.checkpoints.iter().any(|c| c.rest) {
            return Vec::new();
        }
        crate::compiler::combat::hostile_actors(self.campaign)
    }
}
