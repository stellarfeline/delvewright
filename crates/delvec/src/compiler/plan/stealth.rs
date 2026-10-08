//! Stealth: the stealth beat, its active window, and a zone's box.

use super::*;

/// A resolved `begin-stealth` beat (DSL v0.6, spec-0014), collected in
/// deterministic content order; its `index` (1-based) is the active-session id
/// written to `#stealth dw.sys` (0 = inactive).
#[derive(Clone, Debug)]
pub struct StealthBeat {
    /// Stable content-ordered session id (1-based).
    pub index: usize,
    /// Zones: `(anchor name, resolved centre cell, half-extents)`.
    pub zones: Vec<(String, [i32; 3], [u32; 3])>,
    /// Per-player `on_caught` effects (may be empty).
    pub on_caught: Vec<QuestEffect>,
    /// Ticks of exposure tolerated before `on_caught` fires.
    pub grace_ticks: u32,
    /// `critical_path` step index that activates the beat (roots DW0327).
    pub fire_step: usize,
    /// `critical_path` step index at which the beat stops judging players — the
    /// firing step of the first `end-stealth` after `fire_step`, or of the next
    /// `begin-stealth` (a new session replaces the running one), whichever comes
    /// first. `None` = the beat is never closed and runs to the end of the
    /// campaign. Roots the DW0355 onset proof's respawn-position set: a
    /// checkpoint reigning anywhere in `[fire_step, end_step]` can drop a player
    /// into this beat.
    pub end_step: Option<usize>,
}

impl StealthBeat {
    /// Whether being caught in this beat actually **punishes** the player — the
    /// `on_caught` tree contains a `damage-players` (direct harm) or a
    /// `spawn-wave` (hostile mobs). A beat that only narrates has nothing to
    /// escape from, so the DW0355 onset-survivability obligation does not apply
    /// to it; a punishing beat must be escapable from every position a player can
    /// legally occupy when it starts.
    pub fn is_punishing(&self) -> bool {
        fn punishing(eff: &QuestEffect) -> bool {
            if matches!(
                &eff.verb,
                Verb::DamagePlayers { .. } | Verb::SpawnWave { .. }
            ) {
                return true;
            }
            eff.nested_effect_lists()
                .into_iter()
                .flatten()
                .any(punishing)
        }
        self.on_caught.iter().any(punishing)
    }
}

impl<'a> Plan<'a> {
    /// Resolve an anchor-centred box (spec-0022) to absolute inclusive corners:
    /// `anchor ± extent`, the same shape `begin-stealth` zones and
    /// `damage-players`'s `in` filter use. `None` when no placed piece provides
    /// the anchor.
    ///
    /// This — not a prefab `region` anchor — is how the trap-payload verbs
    /// describe a volume, because [`crate::compiler::assembled`] unconditionally CLEARS
    /// every `ResolvedAnchor::Gate` region from the assembled world. A `collapse`
    /// ceiling declared as a region anchor would be deleted at build time, and a
    /// `volley` kill zone would silently punch a hole in the geometry it names.
    pub fn zone_box(&self, zone: &delvewright_dsl::StealthZone) -> Option<([i32; 3], [i32; 3])> {
        zone_box_in(&self.anchors, zone)
    }

    /// The collected stealth beat matching a `begin-stealth` effect (by zone
    /// anchors + `grace_ticks`), giving the emitter its 1-based session id.
    pub fn stealth_for(
        &self,
        zones: &[delvewright_dsl::StealthZone],
        grace: u32,
    ) -> Option<&StealthBeat> {
        self.stealth_beats.iter().find(|b| {
            b.grace_ticks == grace
                && b.zones.len() == zones.len()
                && b.zones
                    .iter()
                    .zip(zones)
                    .all(|((a, _, e), z)| a.as_str() == z.anchor.as_str() && *e == z.extent)
        })
    }
}

/// Close every beat's active window: a running session ends at the first
/// `end-stealth` fired after it, or when the next `begin-stealth` replaces it
/// (`#stealth dw.sys` holds ONE session id), whichever is earlier. A beat with
/// neither runs to the end of the campaign (`None`). Deterministic: driven by the
/// content-ordered collections only.
pub(super) fn close_stealth_windows(beats: &mut [StealthBeat], ends: &[usize]) {
    let fires: Vec<usize> = beats.iter().map(|b| b.fire_step).collect();
    for (i, beat) in beats.iter_mut().enumerate() {
        let after = |s: &usize| *s > beat.fire_step;
        let first_end = ends.iter().filter(|s| after(s)).min().copied();
        let next_begin = fires.iter().skip(i + 1).filter(|s| after(s)).min().copied();
        beat.end_step = match (first_end, next_begin) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
    }
}

/// Resolve an anchor-centred box (`anchor ± extent`) over a resolved-anchor map —
/// the free-function core of [`Plan::zone_box`], for the same reason
/// [`point_any_in`] exists.
pub(crate) fn zone_box_in(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    zone: &delvewright_dsl::StealthZone,
) -> Option<([i32; 3], [i32; 3])> {
    let c = point_any_in(anchors, zone.anchor.as_str())?;
    let e = zone.extent;
    Some((
        [c[0] - e[0] as i32, c[1] - e[1] as i32, c[2] - e[2] as i32],
        [c[0] + e[0] as i32, c[1] + e[1] as i32, c[2] + e[2] as i32],
    ))
}
