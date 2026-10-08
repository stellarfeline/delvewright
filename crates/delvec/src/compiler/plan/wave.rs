//! Waves: a wave's record, its size and the seats its mobs take.

use super::*;

/// A stage-5 wave by id (v0.3).
pub fn wave_of<'a>(campaign: &'a Campaign, wave_id: &str) -> Option<&'a delvewright_dsl::Wave> {
    campaign
        .quests
        .content
        .waves
        .iter()
        .find(|w| w.id.as_str() == wave_id)
}
/// A wave's total mob count.
pub fn wave_total(wave: &delvewright_dsl::Wave) -> i32 {
    wave.mobs.iter().map(|m| m.count as i32).sum()
}

/// **Which body lands on which seated cell** — the wave's declared stacks paired
/// with the cells `emit::plan_wave_spawns` chose, in declaration order, each
/// stack taking `count` of them.
///
/// The convention is the emitter's: `spawn_<wave>` walks the stacks in order and
/// takes `cells[idx]` once per mob. Every PROOF about a seated body asks here
/// instead of walking it again — the winnability pass did have its own copy, and
/// the lethal-volume seat proof was about to be the third. Three walks over one
/// indexing convention is three chances to disagree about which mob a diagnostic
/// is talking about, and the disagreement would be silent: the cells are right
/// either way, only the name attached to them moves.
///
/// A wave with more mobs than seats yields only the seats there are: the
/// shortfall is `DW0312`'s finding, made where the seats are chosen, and not
/// this function's to restate.
pub fn wave_seats<'a>(
    wave: &'a delvewright_dsl::Wave,
    cells: &[[i32; 3]],
) -> Vec<(&'a str, [i32; 3])> {
    let mut out = Vec::new();
    let mut seat = 0usize;
    for mob in &wave.mobs {
        for _ in 0..mob.count {
            if let Some(cell) = cells.get(seat) {
                out.push((mob.entity.as_str(), *cell));
            }
            seat += 1;
        }
    }
    out
}

/// The area a wave's mobs spawn in — `None` for a wave no beat seats. The one
/// definition lives with the fight classes in the DSL
/// ([`delvewright_dsl::wave_area`]), because the document-tier `on_kill` rule
/// (`DW0913`) reads the same fact before any build.
pub use delvewright_dsl::wave_area;

impl<'a> Plan<'a> {
    /// The waves a bonfire rest / bonfire respawn re-seats (spec-0016 §1), in
    /// content order. Empty unless the campaign declares BOTH a `bonfire` and at
    /// least one wave with `respawns_on_rest` — `DW0370` rejects the half that
    /// declares the field without a bonfire, so this is empty exactly for
    /// campaigns that use none of the surface (byte-identical emission).
    pub fn reseat_waves(&self) -> Vec<&delvewright_dsl::Wave> {
        if !self.checkpoints.iter().any(|c| c.rest) {
            return Vec::new();
        }
        self.campaign
            .quests
            .content
            .waves
            .iter()
            .filter(|w| w.respawns_on_rest)
            .collect()
    }

    /// The waves a bonfire refreshes **only while they are undefeated**
    /// (spec-0016 §1): every `elite`/`boss`-tier wave
    /// that does NOT declare `respawns_on_rest`, in content order.
    ///
    /// The distinction from [`Self::reseat_waves`] is the whole ruling. A
    /// `respawns_on_rest` wave comes back *whether or not* the party beat it —
    /// the fire is not a progress ratchet. A billed elite/boss does not: beat it
    /// and it stays beaten (spec-0016 §1, "stage bosses never respawn on rest").
    /// But while it is still standing, chipping it down one hit per life is never
    /// a valid path, so a rest wipes what is left of it and re-seats the authored
    /// wave at full count and full health. The two sets are disjoint by
    /// construction here, so no wave can be re-seated twice by one rest;
    /// `DW0499` forbids the `boss` + `respawns_on_rest` combination outright.
    ///
    /// Empty without a bonfire, and empty for every campaign that bills no
    /// encounter → byte-identical emission.
    pub fn undefeated_reseat_waves(&self) -> Vec<&delvewright_dsl::Wave> {
        if !self.checkpoints.iter().any(|c| c.rest) {
            return Vec::new();
        }
        self.campaign
            .quests
            .content
            .waves
            .iter()
            .filter(|w| !w.respawns_on_rest)
            .filter(|w| {
                w.tier
                    .is_some_and(delvewright_dsl::EncounterTier::has_floor_expectation)
            })
            .collect()
    }
}
