//! Cutscene rehearsal inventory (spec-0019): the compile-time enumeration of
//! every rehearsable **beat** (an effect bundle containing a cutscene) and every
//! **shot** inside it, each carrying the JSON pointer that names it in the
//! `quests` stage document plus the compiled camera geometry the creator overlay
//! bakes into `dw:rehearsal` storage as the starting proposal.
//!
//! ## Why block cells, not world points
//!
//! A camera waypoint in the DSL is `anchor + integer offset`, and
//! [`crate::nav::anchor_offset_point`] resolves it to that cell's **centre**
//! (`cell + 0.5` on every axis). Calibration writes back `anchor + integer
//! offset` and nothing else (spec-0019 §5), so any sub-block precision in a
//! proposal is discarded at write-back by construction. The proposal therefore
//! stores **integer block cells** end to end:
//!
//! - it is exactly the DSL's own granularity, so the round trip
//!   *cell → anchor + offset → cell* is lossless (zero snapping error);
//! - every value that crosses into an mcfunction macro is an integer, whose
//!   SNBT form carries no type suffix — a `double` would substitute as `12.5d`
//!   and produce an unparseable `tp`/`say` argument.
//!
//! The overlay reconstructs the world point the compiler would have used
//! (`cell + 0.5`), so a rehearsed shot and a shipped shot name the same points.
//!
//! ## Determinism
//!
//! The traversal is a pure function of the campaign document in declaration
//! order (`BTreeMap` for `on_objective_complete`, `Vec` order everywhere else);
//! ids are assigned by that order. No RNG, no wall clock, no hash-order
//! iteration (ADR-0006).

use delvewright_dsl::{CameraShot, Campaign, QuestEffect};

use crate::camera::{self, AimTrack, MoveCtx};
use crate::nav::{ActorMovePlan, MovePlan};
use crate::plan::Plan;

/// Upper bound on the waypoints a styled shot contributes to its default
/// proposal. A follow style (`side-track` / `low-follow`) expands to a *per
/// tick* camera track — up to 401 points — which is a camera solve, not an
/// authored polyline. The proposal seeds from a uniform sample of it so the
/// creator has something to replay and re-mark, never the raw track.
const MAX_DEFAULT_WAYPOINTS: usize = 8;

/// One rehearsable shot: what `dw.mark` / `dw.aim` / `dw.faster` / `dw.slower`
/// address and what `dw.done` stamps.
#[derive(Clone, Debug, PartialEq)]
pub struct RehearsalShot {
    /// **1-based** id: the `<s>` in `/trigger dw.mark set <s>` and the `shot=`
    /// field of the `[DelveShot]` stamp. One-based because the reset spelling
    /// is `set -<s>` and `-0 == 0` would be indistinguishable from shot 0.
    pub id: usize,
    /// 1-based id of the beat (effect bundle) this shot belongs to.
    pub beat: usize,
    /// JSON pointer to the **`cutscene` effect** in the `quests` stage document.
    ///
    /// Deliberately the effect, not the shot: the single-shot spelling
    /// (`{path, seconds}`) and its one-entry `shots` equivalent are the same
    /// cutscene and must emit byte-identical output (pinned by
    /// `v06_cutscene::single_shot_spellings_are_byte_identical`), so the id a
    /// shot carries cannot depend on which spelling was used. [`Self::shot_index`]
    /// says which shot of the effect this is; together they name the DSL node a
    /// patch applies to under either spelling.
    pub pointer: String,
    /// This shot's 0-based index within its `cutscene` effect. A patch applies
    /// at `<pointer>/shots/<shot_index>` under the multi-shot spelling, and at
    /// `<pointer>` itself under the single-shot one (index is always 0 there).
    pub shot_index: usize,
    /// Compiled camera waypoints as block cells.
    pub path: Vec<[i32; 3]>,
    /// Compiled look target cell, when the shot resolves to a fixed one.
    pub look_at: Option<[i32; 3]>,
    /// Compiled duration in seconds.
    pub seconds: u32,
}

/// One rehearsable beat: an effect bundle that plays at least one cutscene.
#[derive(Clone, Debug, PartialEq)]
pub struct RehearsalBeat {
    /// 1-based id (the `<b>` in `/trigger dw.beat set <b>`).
    pub id: usize,
    /// JSON pointer to the effect **list**.
    pub pointer: String,
}

/// The campaign's full rehearsal inventory, in emission order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inventory {
    /// Every beat, in traversal order.
    pub beats: Vec<RehearsalBeat>,
    /// Every shot, in traversal order.
    pub shots: Vec<RehearsalShot>,
}

impl Inventory {
    /// True when the campaign has nothing to rehearse (the overlay then emits
    /// no rehearsal artifacts at all, so a cutscene-less campaign's creator
    /// overlay is byte-identical to its pre-spec-0019 form).
    pub fn is_empty(&self) -> bool {
        self.shots.is_empty()
    }
}

/// Build the inventory for `plan`.
///
/// `moves` / `actor_moves` are the compiler's planned walk tracks — the same
/// ones `emit` feeds [`camera::expand_shot`], so a styled shot's default
/// proposal is the geometry that actually ships.
pub fn inventory(plan: &Plan, moves: &[MovePlan], actor_moves: &[ActorMovePlan]) -> Inventory {
    let campaign = plan.campaign;
    // Move context per cutscene effect, keyed by identity: `camera::cutscene_units`
    // owns the scope rules (sibling moves, `sequence` timelines, reaction lists),
    // and re-deriving them here would be a second, drifting copy.
    let units = camera::cutscene_units(campaign);
    let ctx_of = |eff: &QuestEffect| -> Vec<MoveCtx> {
        units
            .iter()
            .find(|(e, _)| std::ptr::eq(*e, eff))
            .map(|(_, c)| c.clone())
            .unwrap_or_default()
    };

    let mut inv = Inventory::default();
    for (pointer, effects) in bundles(campaign) {
        let mut found: Vec<(String, &QuestEffect)> = Vec::new();
        for (i, eff) in effects.iter().enumerate() {
            descend(format!("{pointer}/{i}"), eff, &mut found);
        }
        let cutscenes: Vec<(String, &QuestEffect)> = found
            .into_iter()
            .filter(|(_, e)| {
                matches!(e, QuestEffect::Cutscene { .. })
                    && e.cutscene_shots().is_some_and(|s| !s.is_empty())
            })
            .collect();
        if cutscenes.is_empty() {
            continue;
        }
        let beat = inv.beats.len() + 1;
        inv.beats.push(RehearsalBeat {
            id: beat,
            pointer: pointer.clone(),
        });
        for (eff_ptr, eff) in cutscenes {
            let shots = eff.cutscene_shots().unwrap_or_default();
            let ctx = ctx_of(eff);
            let mut offset: i32 = 0;
            for (k, shot) in shots.iter().enumerate() {
                let expanded = camera::expand_shot(plan, moves, actor_moves, shot, &ctx, offset);
                offset += expanded.ticks + 1;
                inv.shots.push(RehearsalShot {
                    id: inv.shots.len() + 1,
                    beat,
                    pointer: eff_ptr.clone(),
                    shot_index: k,
                    path: default_path(&expanded),
                    look_at: default_look_at(shot, &expanded),
                    seconds: shot.resolved_seconds(),
                });
            }
        }
    }
    inv
}

/// Every effect bundle of the campaign as `(json pointer to the list, effects)`,
/// in the order `emit` walks them.
fn bundles(campaign: &Campaign) -> Vec<(String, &[QuestEffect])> {
    // The roots are inherited, not re-listed. This function used to be a literal
    // hand-rolled copy of the root enumeration minus `traps[].payload` and the
    // dialogue `on_respawn` bundle, while its own doc claimed it walked "in the
    // order `emit` walks them" — a claim that had already been falsified once.
    let mut out: Vec<(String, &[QuestEffect])> = Vec::new();
    crate::plan::for_each_effect_root(campaign, &mut |site, list| {
        out.push((site.path.clone(), list));
    });
    out
}

/// Depth-first walk of one effect and every list nested under it, recording the
/// JSON pointer of each node (the same descent `check_effect_anchors` uses).
fn descend<'a>(path: String, eff: &'a QuestEffect, out: &mut Vec<(String, &'a QuestEffect)>) {
    out.push((path.clone(), eff));
    for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
        for (j, inner) in list.iter().enumerate() {
            descend(format!("{path}/{pseg}/{j}"), inner, out);
        }
    }
}

/// The default proposal path: the shot's resolved geometry as block cells.
///
/// A world point `p` produced by [`crate::nav::anchor_offset_point`] is exactly
/// `cell + 0.5`, so `round(p - 0.5)` recovers the authored cell bit-for-bit; for
/// a style-expanded point it names the cell whose centre is nearest. A per-tick
/// follow track is uniformly down-sampled to [`MAX_DEFAULT_WAYPOINTS`] (endpoints
/// always kept) and consecutive duplicates collapsed.
fn default_path(expanded: &camera::ExpandedShot) -> Vec<[i32; 3]> {
    let pts = expanded.clip_polyline();
    if pts.is_empty() {
        return vec![[0, crate::plan::BASE_Y, 0]];
    }
    let sampled: Vec<[f64; 3]> = if pts.len() <= MAX_DEFAULT_WAYPOINTS {
        pts.to_vec()
    } else {
        let last = pts.len() - 1;
        (0..MAX_DEFAULT_WAYPOINTS)
            .map(|i| pts[i * last / (MAX_DEFAULT_WAYPOINTS - 1)])
            .collect()
    };
    let mut cells: Vec<[i32; 3]> = Vec::new();
    for p in sampled {
        let cell = to_cell(p);
        if cells.last() != Some(&cell) {
            cells.push(cell);
        }
    }
    cells
}

/// The default proposal look target: the shot's explicit `look_at`, or a
/// style-resolved **static** aim. A moving-subject or travel aim has no fixed
/// point and is left unset (`dw.aim` can give it one).
fn default_look_at(shot: &CameraShot, expanded: &camera::ExpandedShot) -> Option<[i32; 3]> {
    if shot.look_at.is_none() && !matches!(expanded.aim, AimTrack::Static(_)) {
        return None;
    }
    match expanded.aim {
        AimTrack::Static(p) => Some(to_cell(p)),
        _ => None,
    }
}

/// The block cell whose centre is nearest world point `p`.
fn to_cell(p: [f64; 3]) -> [i32; 3] {
    [
        (p[0] - 0.5).round() as i32,
        (p[1] - 0.5).round() as i32,
        (p[2] - 0.5).round() as i32,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cell centre round-trips exactly: the emission convention
    /// (`cell + 0.5`) is inverted without drift, so a compiled waypoint's
    /// proposal default IS the authored cell.
    #[test]
    fn cell_centres_round_trip_exactly() {
        for c in [[0, 64, 0], [-13, 70, 41], [2047, -60, -2047]] {
            let p = [c[0] as f64 + 0.5, c[1] as f64 + 0.5, c[2] as f64 + 0.5];
            assert_eq!(to_cell(p), c);
        }
    }

    /// A style-expanded point that is not a cell centre snaps to the cell whose
    /// centre is nearest — never silently to the floor.
    #[test]
    fn arbitrary_points_snap_to_the_nearest_cell_centre() {
        assert_eq!(to_cell([3.9, 64.2, -7.1]), [3, 64, -8]);
        assert_eq!(to_cell([3.4, 64.6, -6.9]), [3, 64, -7]);
    }
}
