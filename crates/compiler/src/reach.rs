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
//! * the **completion volume** the datapack tests — from v0.3 an axis-aligned
//!   block region about the anchor cell ([`ReachCompletion::Cube`]); at v0.2 a
//!   `distance=..radius` sphere about the anchor point
//!   ([`ReachCompletion::Sphere`]);
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
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::failure::Failure;
use crate::nav::{LegRoute, SNAP_RADIUS, World};
use crate::plan::{Plan, ResolvedAnchor, Step};
use delvewright_dsl::stages::Objective;
use delvewright_dsl::{DwCode, ExitTier, envelope::is_v03};

/// `DW0850`: **a `reach` the party can arrive at without completing.**
///
/// Either nothing in the completion volume is a cell a body can stand in, or the
/// footing the route proof delivers the party to lies outside it. Both are the
/// same sentence about the object class — *the place that completes this and the
/// place a body can be are the same place* — and they are one code because the
/// remedy is the same: move the anchor onto the footing, or give the footing to
/// the anchor. Nudging the waypoint is never the fix; the waypoint is where the
/// world put it.
pub const DW_REACH_UNCOMPLETABLE: DwCode = DwCode::every_version("DW0850", ExitTier::Build);

/// `DW0881`: **a raised `reach` anchor completes from the floor below it.**
///
/// The completion volume is centred on the anchor cell in all three axes, and
/// vanilla adjudicates the selector against the body's whole AABB rather than
/// against the cell its feet are in. A standing player is
/// [`delvewright_dsl::metrics::PLAYER_HEIGHT`] tall, so a body on a floor one
/// course below the volume's bottom layer already reaches into it. Put those two
/// facts together and ANY raised anchor whose radius reaches a lower floor
/// completes from that floor: the party never climbs, and the beat fires during
/// whatever they were doing down there.
///
/// The rule this refuses under is stated over the FOOTPRINT rather than over the
/// radius, because a number is the wrong thing to bound — a radius that is
/// generous over a flat plaza and one that reaches through a mezzanine floor are
/// the same number. The footprint is every standable cell a body could complete
/// from ([`ReachCompletion::possibly_completes_from`]), and the demand is that
/// every cell of it can walk to the anchor's own footing **without leaving the
/// footprint**. A ramp or a stair inside the volume satisfies that and is meant
/// to: a body on it is arriving. A hall floor three courses down does not,
/// because nothing inside the volume joins the two floors.
///
/// **Directional, and deliberately so.** The question is whether a body standing
/// on the offending cell can get to the anchor, not whether the anchor can get to
/// it — a body can fall off a loft into the hall below, and that a body could
/// arrive and then leave says nothing about the party who walked in at the bottom
/// and never climbed. So the walk is run backwards from the footing over
/// [`World::neighbors`], the engine's one step rule.
///
/// A code of its own rather than a [`DW_REACH_UNCOMPLETABLE`] variant, for the
/// reason `DW0510` is one: `DW0850` says the objective cannot be completed and
/// sends the author to look for missing footing. This says the opposite — the
/// objective completes too easily, from a place that is not the place — and the
/// remedy is the radius or the anchor, never more floor.
pub const DW_REACH_OFF_FLOOR: DwCode = DwCode::every_version("DW0881", ExitTier::Build);

/// The volume a body has to be in for a `reach` objective to complete.
///
/// Constructed only by [`reach_completion`], which is the one place the rule is
/// written down. Carried verbatim by [`crate::plan::Step::Reach`] into
/// `critical-path.json`, formatted into the `tick` selector by
/// [`ReachCompletion::selector_args`], and judged by [`judge_reach_completion`]
/// below — three readers of one value, none of them re-deriving it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReachCompletion {
    /// Pre-v0.3: `distance=..radius` about the anchor's block corner. Kept
    /// because v0.2 campaigns emit it and must stay byte-identical, and because
    /// the instance behind `DW0850` happened inside exactly this arm.
    Sphere {
        /// The anchor point the distance is measured from.
        pos: [i32; 3],
        /// The authored completion radius, in blocks.
        radius: u32,
    },
    /// v0.3+: an axis-aligned block region about the anchor cell, inclusive
    /// corners. Emitted as `x=<lo.x>,dx=<hi.x-lo.x>,…`, and vanilla's `dx=n`
    /// spans `n + 1` block columns, so the two agree by construction.
    Cube {
        /// Inclusive low corner.
        lo: [i32; 3],
        /// Inclusive high corner.
        hi: [i32; 3],
    },
}

/// The completion volume for one `reach-anchor`, given the resolved anchor cell,
/// the authored radius, and whether the campaign's quests stage is (`v03`) at or
/// above 0.3.0.
///
/// **The one place this rule is written.**
pub fn reach_completion(pos: [i32; 3], radius: u32, v03: bool) -> ReachCompletion {
    if !v03 {
        return ReachCompletion::Sphere { pos, radius };
    }
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
            ReachCompletion::Sphere { pos, radius } => {
                format!("x={},y={},z={},distance=..{radius}", pos[0], pos[1], pos[2])
            }
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
            ReachCompletion::Sphere { pos, radius } => {
                serde_json::json!({ "kind": "sphere", "pos": pos, "radius": radius })
            }
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
            ReachCompletion::Sphere { pos, radius } => {
                // A body standing in cell `c` has its feet at the cell's centre
                // column, at the cell's own floor height — the position vanilla
                // measures `distance` from. The anchor point is the raw
                // coordinate triple the selector carries.
                let dx = (c[0] as f64 + 0.5) - pos[0] as f64;
                let dy = c[1] as f64 - pos[1] as f64;
                let dz = (c[2] as f64 + 0.5) - pos[2] as f64;
                (dx * dx + dy * dy + dz * dz).sqrt() <= radius as f64
            }
        }
    }

    /// Could a body standing in cell `c`, with its feet at `feet_y` blocks, complete
    /// here **at all**?
    ///
    /// The generous counterpart of [`ReachCompletion::certainly_completes_from`],
    /// and the two are named apart because the direction of their looseness decides
    /// what each may be used for. `certainly` answers *does the party complete this*
    /// and so must not credit a face-touching tie; this answers *could anybody
    /// complete this from here*, which is the question a rule that REFUSES a
    /// completion has to ask — a generous reading is safe for the refusal and a
    /// conservative one would let exactly the unwanted cells through.
    ///
    /// The cube arm is the vanilla test written out. `x=X,dx=N` builds the AABB
    /// `[X, X+N+1]` (`EntitySelectorParser::createAabb` adds the block's own extent
    /// on the positive side, which is why `dx=0` selects one block column), and
    /// `getEntities` keeps every entity whose own box **intersects** it. So the body
    /// box — [`delvewright_dsl::metrics::PLAYER_WIDTH`] square about the cell centre,
    /// [`delvewright_dsl::metrics::PLAYER_HEIGHT`] tall from the feet — is
    /// intersected against it directly. The feet are handed in rather than assumed
    /// to be the cell floor, because a partial support puts them lower
    /// ([`World::feet_y`]).
    ///
    /// The sphere arm is exact rather than generous, and that is a fact about
    /// vanilla, not a shortcut: `distance` measures from the entity's POSITION,
    /// which is a point at the feet, so there is no hitbox for a tolerance to live
    /// in.
    pub fn possibly_completes_from(&self, c: [i32; 3], feet_y: f64) -> bool {
        match *self {
            ReachCompletion::Cube { lo, hi } => {
                let half = delvewright_dsl::metrics::PLAYER_WIDTH / 2.0;
                let body_lo = [c[0] as f64 + 0.5 - half, feet_y, c[2] as f64 + 0.5 - half];
                let body_hi = [
                    c[0] as f64 + 0.5 + half,
                    feet_y + delvewright_dsl::metrics::PLAYER_HEIGHT,
                    c[2] as f64 + 0.5 + half,
                ];
                (0..3).all(|i| body_lo[i] < f64::from(hi[i] + 1) && body_hi[i] > f64::from(lo[i]))
            }
            ReachCompletion::Sphere { pos, radius } => {
                let dx = (c[0] as f64 + 0.5) - f64::from(pos[0]);
                let dy = feet_y - f64::from(pos[1]);
                let dz = (c[2] as f64 + 0.5) - f64::from(pos[2]);
                (dx * dx + dy * dy + dz * dz).sqrt() <= f64::from(radius)
            }
        }
    }

    /// Every cell [`ReachCompletion::possibly_completes_from`] could answer yes
    /// about, before the world is asked anything.
    ///
    /// One cell of slack around the volume in every direction, and the slack is
    /// derived rather than chosen: a body's feet sit between `c.y - 1` and `c.y`
    /// (a partial support drops them, nothing raises them), and its box rises
    /// [`delvewright_dsl::metrics::PLAYER_HEIGHT`] from there, so a standing cell as
    /// low as one below the volume's bottom layer still reaches into it and one as
    /// high as one above the top layer can still have its feet inside. Horizontally
    /// the body is narrower than its column today, so the slack there buys nothing
    /// and is kept because the width is a metric and not a constant of this file.
    fn footprint_candidates(&self) -> Vec<[i32; 3]> {
        let (min, max) = match *self {
            ReachCompletion::Cube { lo, hi } => (lo, hi),
            ReachCompletion::Sphere { pos, radius } => {
                let r = radius as i32 + 1;
                (
                    [pos[0] - r, pos[1] - r, pos[2] - r],
                    [pos[0] + r, pos[1] + r, pos[2] + r],
                )
            }
        };
        let mut out = Vec::new();
        for x in min[0] - 1..=max[0] + 1 {
            for y in min[1] - 1..=max[1] + 1 {
                for z in min[2] - 1..=max[2] + 1 {
                    out.push([x, y, z]);
                }
            }
        }
        out
    }

    /// Every cell a body could stand in and certainly complete. Bounded: the
    /// sphere arm is enumerated over its own integer bounding box.
    pub fn cells(&self) -> Vec<[i32; 3]> {
        let (min, max) = match *self {
            ReachCompletion::Cube { lo, hi } => (lo, hi),
            ReachCompletion::Sphere { pos, radius } => {
                let r = radius as i32 + 1;
                (
                    [pos[0] - r, pos[1] - r, pos[2] - r],
                    [pos[0] + r, pos[1] + r, pos[2] + r],
                )
            }
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
            ReachCompletion::Sphere { pos, radius } => {
                format!("the sphere of radius {radius} about {pos:?}")
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

/// Whether this campaign emits the v0.3+ cube form. Reads the quests stage, the
/// same gate `emit::campaign_is_v03` reads, because it is the same decision.
fn is_cube_campaign(plan: &Plan) -> bool {
    is_v03(plan.campaign.quests.dsl_version.as_str())
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
    let v03 = is_cube_campaign(plan);
    for site in sites(plan) {
        let vol = reach_completion(site.pos, site.radius, v03);

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

/// What `DW0881` examined, stated on every build whether it found anything or not.
///
/// Three numbers, because a zero in each means a different thing and only the
/// first is ever a design: **no reach objective at all** is a campaign that never
/// asks anybody to arrive anywhere; **footprint cells zero over objectives that
/// exist** means every completion volume in this campaign is empty air, which
/// `DW0850` is the one to say so; and **off-floor zero** is the pass this check is
/// for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReachFootprintBinding {
    /// `reach` objectives whose anchor the plan resolved.
    pub sites: usize,
    /// The denominator: cells the completion volumes cover, before the world is
    /// asked anything. A footprint count means nothing without the population it
    /// was drawn from — and a zero here with a non-zero `sites` is a volume
    /// enumeration that stopped enumerating.
    pub candidates: usize,
    /// The other denominator: cells the party can walk to from the campaign's own
    /// starting cell. A zero here over a campaign that declares a start is a world
    /// nobody can move in, and every verdict below is then vacuous rather than
    /// clean.
    pub standing: usize,
    /// Standable cells across all of them from which a body could complete.
    pub cells: usize,
    /// Of those, the ones no body can walk to the anchor's own footing from
    /// without leaving the completion volume.
    pub off_floor: usize,
}

impl ReachFootprintBinding {
    /// The one line this proof owes its reader.
    pub fn line(&self) -> String {
        format!(
            "reach-footprint binding: {} reach objective(s) examined over {} cell(s) their \
             completion volumes cover, against {} cell(s) the party can stand on; {} of those \
             are footing a body could complete from, and {} of THOSE stand on floor no body \
             can walk to the anchor's own footing from without leaving the completion volume.",
            self.sites, self.candidates, self.standing, self.cells, self.off_floor
        )
    }
}

/// Where the anchor's OWN footing is: the cell a body stands in when it has
/// arrived at `pos`.
///
/// Asked of the world in the order every other proof in this compiler asks it —
/// the anchor cell itself, then [`World::snap`] at [`SNAP_RADIUS`], which is what
/// `check_critical_path` routes to — and only then, when neither answer is a cell
/// that could complete, the nearest footprint cell by `(distance², cell)`. The
/// fallback exists because the reference has to be inside the footprint for the
/// walk below to mean anything, and it is deterministic (ADR-0006).
fn anchor_footing(world: &World, pos: [i32; 3], cells: &BTreeSet<[i32; 3]>) -> Option<[i32; 3]> {
    if cells.contains(&pos) {
        return Some(pos);
    }
    if let Some(s) = world.snap(pos, SNAP_RADIUS)
        && cells.contains(&s)
    {
        return Some(s);
    }
    cells.iter().copied().min_by_key(|c| {
        let d2: i64 = (0..3)
            .map(|i| {
                let d = i64::from(c[i] - pos[i]);
                d * d
            })
            .sum();
        (d2, *c)
    })
}

/// The cells of `cells` from which a body can walk to `footing` without ever
/// leaving `cells`.
///
/// Run BACKWARDS: the forward step graph is built over the footprint with
/// [`World::neighbors`] — the engine's one step rule, so this walk and every
/// routed leg agree about what a body can do — and then inverted, so the flood
/// from the footing collects everything that can reach IT. Forwards would answer a
/// different question (what the anchor can get to), and a one-way drop off the
/// loft into the hall would then excuse the hall floor.
///
/// Deterministic: `BTreeMap`/`BTreeSet` throughout and `neighbors`' own fixed
/// order (ADR-0006).
fn cells_that_reach(
    world: &World,
    cells: &BTreeSet<[i32; 3]>,
    footing: [i32; 3],
) -> BTreeSet<[i32; 3]> {
    let mut pred: BTreeMap<[i32; 3], Vec<[i32; 3]>> = BTreeMap::new();
    for &from in cells {
        for to in world.neighbors(from) {
            if cells.contains(&to) {
                pred.entry(to).or_default().push(from);
            }
        }
    }
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    seen.insert(footing);
    let mut queue: VecDeque<[i32; 3]> = VecDeque::new();
    queue.push_back(footing);
    while let Some(c) = queue.pop_front() {
        for &p in pred.get(&c).map(Vec::as_slice).unwrap_or(&[]) {
            if seen.insert(p) {
                queue.push_back(p);
            }
        }
    }
    seen
}

/// How many offending cells a floor names before the diagnostic says "and N more".
///
/// The list is what an author walks to; the count is what tells them how much of
/// the floor is affected. Both are needed, and a message that printed 400 cells
/// would be neither.
const SHOWN_PER_FLOOR: usize = 6;

/// `DW0881`: **everywhere a `reach` completes from is somewhere a body arrived
/// at.**
///
/// The footprint of one reach objective is every standable cell whose body box
/// meets the completion volume. The rule is that every cell of it can walk to the
/// anchor's own footing without leaving the footprint — see [`DW_REACH_OFF_FLOOR`]
/// for why that is the rule and not a bound on `radius`.
///
/// **The population is rooted at the party's own starting cell, and which root is
/// used is the whole difference between a rule and a false alarm.** An offender
/// has to be somewhere the party can already stand: `standing` is the walk from
/// `entry` over this same assembled world, so a roof, a canopy or the top of a
/// stamped block that a generous radius happens to cover is not reported, and
/// neither is floor on the far side of a doorway whose planks a beat lays at
/// runtime — before that beat nobody is over there, and the beat that puts them
/// there is the same one that joins the two inside the volume.
///
/// Rooting it at the ANCHOR's footing instead is the version that looks equally
/// reasonable and is wrong, measured rather than argued: the gallery's mezzanine
/// ships with its flight broken and laid by an `open-way`, so in the assembled
/// world the loft is severed from the hall it overlooks — and a population walked
/// from the loft therefore drops the hall floor, the very floor the party
/// completes from. Zero offenders where there are nine. A root whose failure mode
/// is silence on the motivating case fails in the direction nothing downstream
/// re-checks.
///
/// The residual, named rather than implied: a cell the party reaches only after a
/// runtime write is outside `standing` and so outside this rule. That is the
/// conservative direction and it is bounded by the same write — whatever joins the
/// party to such a cell is a change to the geometry the volume is measured
/// against, which no static world can hold two versions of at once.
///
/// Returns the binding beside the verdict rather than short-circuiting on the
/// first refusal, so the line a run prints is a count over every objective and not
/// over the ones that happened to precede the failure.
///
/// Bound to the build at the site that already runs `DW0850`, over the same final
/// assembled world, so there is no path to a datapack that skips it.
pub fn check_reach_footprint(
    plan: &Plan,
    world: &World,
    entry: Option<[i32; 3]>,
) -> (ReachFootprintBinding, Result<(), Failure>) {
    let v03 = is_cube_campaign(plan);
    let standing: BTreeSet<[i32; 3]> = match entry {
        Some(e) => world.reachable_walkable(&[e]),
        // A campaign with no resolvable start has no party to reason about, and
        // an empty population would make every green here vacuous. Say so with a
        // zero binding rather than by passing quietly.
        None => BTreeSet::new(),
    };
    let mut binding = ReachFootprintBinding {
        standing: standing.len(),
        ..ReachFootprintBinding::default()
    };
    let mut first: Option<Failure> = None;
    let mut others: Vec<String> = Vec::new();
    for site in sites(plan) {
        binding.sites += 1;
        let vol = reach_completion(site.pos, site.radius, v03);
        let candidates = vol.footprint_candidates();
        binding.candidates += candidates.len();
        let touching: BTreeSet<[i32; 3]> = candidates
            .into_iter()
            .filter(|&c| world.is_standable(c) && vol.possibly_completes_from(c, world.feet_y(c)))
            .collect();
        // The anchor's own footing is in by construction: the party is proven to
        // reach it by `DW0311`, and a walk that started outside its own root
        // would have nothing to measure connectivity against.
        let cells: BTreeSet<[i32; 3]> = match anchor_footing(world, site.pos, &touching) {
            Some(f) => touching
                .into_iter()
                .filter(|c| *c == f || standing.contains(c))
                .collect(),
            None => touching,
        };
        binding.cells += cells.len();
        let Some(footing) = anchor_footing(world, site.pos, &cells) else {
            // An empty volume is `DW0850`'s finding, stated in its own words at
            // the same site; saying it twice in two vocabularies helps nobody.
            continue;
        };
        let arriving = cells_that_reach(world, &cells, footing);
        let off: Vec<[i32; 3]> = cells.difference(&arriving).copied().collect();
        binding.off_floor += off.len();
        if off.is_empty() {
            continue;
        }
        if first.is_some() {
            // Named, never silently dropped. The failure channel carries one code
            // and one message, which is the compiler's contract; what a reader
            // must not lose is that the campaign has more than one of these, or
            // they fix the first and meet the second on the next build.
            others.push(site.objective_id.clone());
            continue;
        }
        let mut by_y: BTreeMap<i32, Vec<[i32; 3]>> = BTreeMap::new();
        for c in &off {
            by_y.entry(c[1]).or_default().push(*c);
        }
        let floors = by_y
            .iter()
            .map(|(y, cs)| {
                let shown = cs
                    .iter()
                    .take(SHOWN_PER_FLOOR)
                    .map(|c| format!("{c:?}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let rest = cs.len().saturating_sub(SHOWN_PER_FLOOR);
                if rest == 0 {
                    format!("y={y} ({} cell(s): {shown})", cs.len())
                } else {
                    format!(
                        "y={y} ({} cell(s): {shown}, and {rest} more on the same floor)",
                        cs.len()
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("; ");
        first = Some(Failure {
            code: DW_REACH_OFF_FLOOR,
            message: format!(
                "reach objective `{}` completes for any body whose hitbox meets {} at anchor \
                 `{}`, and {} standable cell(s) inside it stand on floor a body cannot walk to \
                 the anchor's own footing {footing:?} from without leaving that volume: {floors}. \
                 Vanilla adjudicates the selector against the whole body box, which rises {} \
                 blocks from the feet, so the volume reaches every floor within a course of it \
                 and a party standing on any of them completes this objective without arriving \
                 at the anchor, while every proof this build runs stays green. The commonest \
                 shape is a raised anchor over a room the party is already in: the beat fires \
                 during whatever they were doing down there and nobody ever climbs. A floor at \
                 the anchor's own height, walled off from it inside the volume, is the same \
                 defect laid flat. `reach` means arriving where the anchor is and the volume is \
                 a tolerance around that, so a tolerance that admits a place you cannot walk to \
                 the anchor from is not a tolerance. Lower `radius` until the volume covers only \
                 the anchor's own floor, or move the anchor so that floor is the only one inside \
                 it. Joining the two INSIDE the volume is the third answer and this rule passes \
                 it: a body on a stair, or on a walk, that the volume covers end to end is \
                 arriving.",
                site.objective_id,
                vol.describe(),
                site.anchor_id,
                off.len(),
                delvewright_dsl::metrics::PLAYER_HEIGHT,
            ),
        });
    }
    if let Some(f) = first.as_mut()
        && !others.is_empty()
    {
        f.message.push_str(&format!(
            " This campaign has {} more reach objective(s) with the same defect, which this \
             channel can only name rather than describe: {}. Fixing the one above will not \
             clear them.",
            others.len(),
            others.join(", ")
        ));
    }
    (binding, first.map_or(Ok(()), Err))
}
