//! What actually completes a `reach` — the single authority for the volume a
//! body has to be in, read by the emitter that writes the selector, by the
//! artifact that hands the volume to the bot, and by the proof that the party
//! can get into it.
//!
//! ## The defect this module exists to make impossible
//!
//! A `reach` objective is the campaign saying *arrive here*. Four separate
//! numbers decide whether arriving works, and until this module nothing ever
//! compared them:
//!
//! * the **completion volume** the datapack tests — an axis-aligned block
//!   region about the anchor cell ([`ReachCompletion::Cube`]);
//! * the **walk goal** `critical-path.json` hands the harness, which used to be
//!   derived from the authored `radius` on the bot's side;
//! * the **footing** the world actually offers near that anchor, which is what
//!   [`crate::nav::World::is_standable`] decides;
//! * the **arrival** the route proof delivers, which is the snapped endpoint of
//!   the leg walking to the anchor — snapped by
//!   [`crate::nav::SNAP_RADIUS`], **three** blocks.
//!
//! Two distinct pairs had drifted, and each drift is invisible in every artifact
//! a board can read.
//!
//! **The volume against the walk goal.** `radius` is authored once. The M2
//! repair for a completion sphere too tight to stand in replaced the sphere with
//! a fixed ±1 cube at v0.3 — and *replaced* is the defect: the authored number
//! stopped reaching the datapack entirely, while the harness went on deriving
//! its walk goal from it and aiming `radius - 1` blocks out, outside the box for
//! every `radius` of 3 or more. The bot stopped short and hung on a completion
//! that could not fire; it stayed green because a `GoalNear` usually overshoots
//! inward, which makes the failure intermittent, and an intermittent failure is
//! an under-specified test. So the v0.3+ half-extent is a **floor** on the
//! authored radius (`max(1, radius)`), never a constant instead of it: the "too
//! tight to stand in" instance stays closed at every radius, and the author's
//! number means what it says again.
//!
//! **The volume against the footing.** The instance behind `DW0850` was repaired
//! by widening the volume once, in the emitter, at v0.3. Nothing re-asserted it
//! on a build, which meant the repair covered the volume that had been reported
//! and no other — and the identical defect reached by a *different* number (snap
//! distance rather than sphere radius) was left live. A reach whose only
//! standable cell is further from the anchor than the volume reaches satisfies
//! every existing proof — `DW0311` finds footing, `DW0314` finds the route
//! standable, the waypoint exports — and the player who walks to the cell the
//! campaign itself routed them to is **outside the volume that completes the
//! objective**. The delve stops there and every board is green.
//!
//! ## Why the volume lives here
//!
//! Three readers must agree about a rule that is invisible in the DSL: the
//! string [`crate::emit`] writes into `tick.mcfunction`, the `completion` field
//! [`crate::plan::Step::Reach`] exports to the harness, and the proof below.
//! Where sites decide one thing independently they eventually disagree, and this
//! particular disagreement is undetectable from any artifact — the selector
//! looks right, the route looks right, and only a human standing on the spot
//! finds out. So there is one function, exactly as
//! [`crate::pressable::body_at`] is the one answer to *what does a click at this
//! anchor land on*. Every reader takes the whole value: the emitter **formats**
//! its selector from it rather than restating an extent beside it, so a change
//! to the rule cannot leave a stale number behind in a `format!`.
use std::collections::BTreeMap;

use crate::failure::Failure;
use crate::nav::{LegRoute, World};
use crate::plan::{Plan, ResolvedAnchor, Step};
use delvewright_dsl::stages::Objective;
use delvewright_dsl::{DwCode, ExitTier};

/// `DW0850`: **a `reach` the party can arrive at without completing.**
///
/// Either nothing in the completion volume is a cell a body can stand in, or the
/// footing the route proof delivers the party to lies outside it. Both are the
/// same sentence about the object class — *the place that completes this and the
/// place a body can be are the same place* — and they are one code because the
/// remedy is the same: move the anchor onto the footing, or give the footing to
/// the anchor. Nudging the waypoint is never the fix; the waypoint is where the
/// world put it.
pub const DW_REACH_UNCOMPLETABLE: DwCode = DwCode::new("DW0850", ExitTier::Build);

/// The volume a body has to be in for a `reach` objective to complete.
///
/// Constructed only by [`reach_completion`], which is the one place the rule is
/// written down. Carried verbatim by [`crate::plan::Step::Reach`] into
/// `critical-path.json`, formatted into the `tick` selector by
/// [`ReachCompletion::selector_args`], and judged by [`judge_reach_completion`]
/// below — three readers of one value, none of them re-deriving it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReachCompletion {
    /// An axis-aligned block region about the anchor cell, inclusive
    /// corners. Emitted as `x=<lo.x>,dx=<hi.x-lo.x>,…`, and vanilla's `dx=n`
    /// spans `n + 1` block columns, so the two agree by construction.
    Cube {
        /// Inclusive low corner.
        lo: [i32; 3],
        /// Inclusive high corner.
        hi: [i32; 3],
    },
}

/// The completion volume for one `reach-anchor`, given the resolved anchor cell
/// and the authored radius.
///
/// **The one place this rule is written.**
pub fn reach_completion(pos: [i32; 3], radius: u32) -> ReachCompletion {
    // The FLOOR, not a replacement: never tighter than the ±1 that closed the
    // "too tight for a standing body" instance, never narrower than what the
    // author asked for.
    let h = radius.max(1) as i32;
    ReachCompletion::Cube {
        lo: [pos[0] - h, pos[1] - h, pos[2] - h],
        hi: [pos[0] + h, pos[1] + h, pos[2] + h],
    }
}

impl ReachCompletion {
    /// The `@s[...]` selector arguments the tick line adjudicates with.
    pub fn selector_args(&self) -> String {
        match self {
            ReachCompletion::Cube { lo, hi } => format!(
                "x={},dx={},y={},dy={},z={},dz={}",
                lo[0],
                hi[0] - lo[0],
                lo[1],
                hi[1] - lo[1],
                lo[2],
                hi[2] - lo[2]
            ),
        }
    }

    /// The same volume as `critical-path.json` carries it, so the harness walks
    /// into the region the server is testing rather than into its own idea of one.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            ReachCompletion::Cube { lo, hi } => {
                serde_json::json!({ "kind": "cube", "lo": lo, "hi": hi })
            }
        }
    }

    /// Does a body whose feet are in cell `c` **certainly** complete here?
    ///
    /// Deliberately the conservative reading of the vanilla test, in the
    /// direction that makes this check demand the guaranteed case. Vanilla tests
    /// hitbox *intersection*, so a body one cell outside the cube may complete on
    /// a face-touching tie that depends on sub-block position; a body inside it
    /// always does. Asking for the certain case is what makes a green here mean
    /// "the party completes this", rather than "the party might".
    pub fn certainly_completes_from(&self, c: [i32; 3]) -> bool {
        match *self {
            ReachCompletion::Cube { lo, hi } => (0..3).all(|i| c[i] >= lo[i] && c[i] <= hi[i]),
        }
    }

    /// Every cell a body could stand in and certainly complete.
    pub fn cells(&self) -> Vec<[i32; 3]> {
        let (min, max) = match *self {
            ReachCompletion::Cube { lo, hi } => (lo, hi),
        };
        let mut out = Vec::new();
        for x in min[0]..=max[0] {
            for y in min[1]..=max[1] {
                for z in min[2]..=max[2] {
                    if self.certainly_completes_from([x, y, z]) {
                        out.push([x, y, z]);
                    }
                }
            }
        }
        out
    }

    /// How the volume reads in a diagnostic.
    ///
    /// The extent is **measured off the value**, never spelled out: the cube used
    /// to be a fixed 3×3×3 and describing it as one would now be a diagnostic
    /// that lies about the volume it is refusing.
    fn describe(&self) -> String {
        match *self {
            ReachCompletion::Cube { lo, hi } => {
                let span = hi[0] - lo[0] + 1;
                format!("the {span}×{span}×{span} cube {lo:?}..={hi:?}")
            }
        }
    }
}

/// One `reach` objective, with the anchor cell it resolved to.
#[derive(Debug, Clone)]
pub struct ReachSite {
    /// `obj/<id>`.
    pub objective_id: String,
    /// `anchor/<id>`.
    pub anchor_id: String,
    /// The resolved anchor cell the completion volume is centred on.
    pub pos: [i32; 3],
    /// The declared completion radius (read only by the v0.2 sphere arm and by
    /// the harness's pathfinder goal).
    pub radius: u32,
}

/// The anchor cell a `reach` on `anchor` in `area` resolves to, or `None` when
/// the plan does not resolve it.
///
/// A gate anchor arrives at its `from` side — the side the party walks up to —
/// which is the same choice [`crate::emit`] makes when it writes the selector.
/// One function, so the two cannot pick different sides.
pub fn anchor_arrival(plan: &Plan, area: &str, anchor: &str) -> Option<[i32; 3]> {
    match plan.anchors.get(&(area.to_string(), anchor.to_string())) {
        Some(ResolvedAnchor::Point { pos, .. }) => Some(*pos),
        Some(ResolvedAnchor::Gate { from, .. }) => Some(*from),
        None => None,
    }
}

/// Every `reach` objective in the campaign whose anchor the plan resolves.
///
/// Quantified over the QUESTS, not over the critical path: a reach on an
/// optional quest is a reach a player can be standing at, and a proof that only
/// examined the exported path would report a binding of "the ones we happened to
/// walk". The critical path is then used, below, only to say what the party is
/// *delivered to* — a strictly extra fact about the subset that has one.
pub fn sites(plan: &Plan) -> Vec<ReachSite> {
    let mut out = Vec::new();
    for q in &plan.campaign.quests.content.quests {
        let area = plan.quest_area(q.id.as_str()).unwrap_or("").to_string();
        for o in &q.objectives {
            let Objective::ReachAnchor {
                id, anchor, radius, ..
            } = o
            else {
                continue;
            };
            let Some(pos) = anchor_arrival(plan, &area, anchor.as_str()) else {
                continue;
            };
            out.push(ReachSite {
                objective_id: id.as_str().to_string(),
                anchor_id: anchor.as_str().to_string(),
                pos,
                radius: *radius,
            });
        }
    }
    out
}

/// `DW0850`: **the volume that completes a `reach`, and the footing a body can
/// reach it from, are the same place.**
///
/// Two assertions per site, in the order a reader needs them:
///
/// 1. **Occupiable.** Some cell of the completion volume is standable in the
///    final assembled world. A volume no body can be in is an objective nothing
///    completes — the finding, exactly, as a property rather than as one altar.
/// 2. **Delivered into.** Where the critical path walks to this anchor, the cell
///    the leg actually ends on is inside the volume. This is the half no
///    existing proof could see: the endpoint snap searches
///    [`crate::nav::SNAP_RADIUS`] blocks and the cube reaches one, so the route
///    can be proven, exported and walked to a cell that does not complete.
///
/// Both are error tier. A delve whose objective cannot be completed by arriving
/// at it is not a build with a style note; it is a delve that stops.
///
/// Bound to the build, at the site that already verifies exported routes — the
/// same event, over the same final world, so there is no path to a shipped
/// datapack that skips it and no checklist line to forget.
pub fn check_reach_completion(
    plan: &Plan,
    world: &World,
    routes: &[LegRoute],
) -> Result<(), Failure> {
    // The one place a leg's ARRIVAL is read off a route: the last cell of the
    // A* polyline, which is the snapped endpoint the walk actually delivers the
    // body to. Split out so the judgement below can be driven from plain data
    // and red-demoed — a `LegRoute` carries a private `region_state` precisely
    // so that only `route_walked_legs` can mint one, and that guarantee is worth
    // more than the convenience of constructing one in a test.
    let arrivals: BTreeMap<usize, [i32; 3]> = routes
        .iter()
        .filter_map(|l| l.cells.last().map(|&c| (l.to_step, c)))
        .collect();
    judge_reach_completion(plan, world, &arrivals)
}

/// The judgement [`check_reach_completion`] makes, over plain data: `arrivals`
/// maps a critical-path step index to the cell the walk delivers the party to.
pub fn judge_reach_completion(
    plan: &Plan,
    world: &World,
    arrivals: &BTreeMap<usize, [i32; 3]>,
) -> Result<(), Failure> {
    for site in sites(plan) {
        let vol = reach_completion(site.pos, site.radius);

        let standable: Vec<[i32; 3]> = vol
            .cells()
            .into_iter()
            .filter(|&c| world.is_standable(c))
            .collect();
        if standable.is_empty() {
            return Err(Failure {
                code: DW_REACH_UNCOMPLETABLE,
                message: format!(
                    "reach objective `{}` completes only for a body inside {} at anchor `{}`, and \
                     no cell of that volume is standable in the final assembled world — so \
                     arriving cannot complete it, however the party gets there. The completion \
                     volume and the footing are the same question and this campaign answers it \
                     two ways. Fix the geometry or move the anchor onto the floor; never widen \
                     the volume to reach the body, which is how this defect was closed once \
                     before and left every other instance live.",
                    site.objective_id,
                    vol.describe(),
                    site.anchor_id,
                ),
            });
        }

        // The critical path's arrival, where there is one. The step index is how
        // a leg names the objective it walks toward, so the arrival is found by
        // the step rather than by re-deriving which leg serves which objective.
        let Some(step_ix) = plan.critical_path.iter().position(
            |s| matches!(s, Step::Reach { objective_id, .. } if objective_id == &site.objective_id),
        ) else {
            continue;
        };
        let Some(&arrival) = arrivals.get(&step_ix) else {
            continue;
        };
        if vol.certainly_completes_from(arrival) {
            continue;
        }
        let nearest = standable
            .iter()
            .map(|c| {
                (0..3)
                    .map(|i| (c[i] - arrival[i]).abs())
                    .max()
                    .unwrap_or(i32::MAX)
            })
            .min()
            .unwrap_or(i32::MAX);
        return Err(Failure {
            code: DW_REACH_UNCOMPLETABLE,
            message: format!(
                "reach objective `{}` completes only for a body inside {} at anchor `{}`, and the \
                 route this build proves and exports delivers the party to {arrival:?}, which is \
                 outside it — {nearest} block(s) from the nearest footing that would complete it. \
                 The endpoint snap searches further than the completion volume reaches, so the \
                 walk is provable, exportable and green while the objective never fires. Move the \
                 anchor onto the footing the party actually arrives on, or give the anchor cell \
                 standable floor of its own. Do NOT move the waypoint — the waypoint is where the \
                 world put it.",
                site.objective_id,
                vol.describe(),
                site.anchor_id,
            ),
        });
    }
    Ok(())
}
