/// The branch condition a staging effect fires under: the per-effect
/// `requires_flags` / `forbids_flags` gate (DSL v0.6).
///
/// This exists because walk origins **chain** — each leg starts where the body's
/// previous leg left it — and that chain used to be a single flat sequence per
/// body, walked in campaign effect order with no regard for which branch each leg
/// belonged to. Owner playtest, island round 15: choosing to *wait* teleported
/// Eurylochus out of the cave down to the beach and walked him 35 seconds back
/// up, because the `flag/flee`-gated leg to the gangplank — a leg that cannot
/// fire on the branch the player took — had overwritten the origin the
/// `flag/wait`-gated leg to the alcove inherited. `npc/perimedes` had the same
/// defect on the same branch, unreported.
///
/// The rule this type enforces is the compiler's usual one (see
/// [`crate::compiler::continuity`]): chain only from what is **provably** already true.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BranchGate {
    /// Flags that must be set for the effect to fire.
    requires: BTreeSet<String>,
    /// Flags that must be unset for the effect to fire.
    forbids: BTreeSet<String>,
}

impl BranchGate {
    /// The gate an effect carries.
    fn of(eff: &QuestEffect) -> Self {
        Self {
            requires: eff
                .requires_flags()
                .iter()
                .map(|f| f.as_str().to_string())
                .collect(),
            forbids: eff
                .forbids_flags()
                .iter()
                .map(|f| f.as_str().to_string())
                .collect(),
        }
    }

    /// Whether the effect is unconditional (always fires).
    fn is_unconditional(&self) -> bool {
        self.requires.is_empty() && self.forbids.is_empty()
    }

    /// Does this gate provably hold on **every** timeline where `other` fires?
    ///
    /// True when `other`'s conditions are a superset of this one's: a leg gated on
    /// nothing always fired; a leg gated on `flag/flee` has provably fired by the
    /// time another `flag/flee` leg runs. It is deliberately *not* true for two
    /// legs gated on different flags — `flag/wait` does not prove `flag/flee`, so
    /// the flee leg is skipped when chaining into the wait leg, which is exactly
    /// the fix.
    ///
    /// The direction is conservative on purpose. The compiler cannot prove two
    /// flags are mutually exclusive (nothing in the DSL says `flag/wait` and
    /// `flag/flee` cannot both be set), so this never *asserts* that a skipped
    /// leg did not fire — it only declines to assume that it did, and falls back
    /// to the most recent staging the branch does prove. That can only ever move
    /// an origin from "certainly wrong on this branch" to "correct on the branch
    /// the DSL describes"; it cannot invent a route.
    fn implied_by(&self, other: &BranchGate) -> bool {
        self.requires.is_subset(&other.requires) && self.forbids.is_subset(&other.forbids)
    }
}

/// The **driver-name suffix** a branch gate contributes.
///
/// A walk driver is content-keyed by the body and its destination, and that key
/// used to be the whole story. It is not: two beats can legitimately walk the
/// same character to the same mark **from different places**, one per branch —
/// the island's Eurylochus reaches `anchor/gangplank` from the cave if the party
/// flees at the cheese, and from the upper sheep pen if they stay and escape
/// under the rams. Both are correct content; one emitted driver cannot carry
/// both origins, and before this the second beat silently ran the first beat's
/// polyline, teleporting the body across the map to start.
///
/// Including the gate in the key gives each branch its own driver. Unconditional
/// walks contribute the **empty** suffix, so every campaign that never gated a
/// walk keeps byte-identical function names.
pub fn gate_key(eff: &QuestEffect) -> String {
    BranchGate::of(eff).key()
}

impl BranchGate {
    /// A short, deterministic, filename-safe key for this gate ("" when
    /// unconditional). Derived from the sorted flag names, so it cannot depend on
    /// declaration order or map iteration.
    fn key(&self) -> String {
        if self.is_unconditional() {
            return String::new();
        }
        let mut acc: u64 = 0xcbf2_9ce4_8422_2325;
        for (tag, set) in [("+", &self.requires), ("-", &self.forbids)] {
            for f in set {
                for b in tag.bytes().chain(f.bytes()) {
                    acc ^= b as u64;
                    acc = acc.wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
        format!("_b{acc:08x}")
    }
}

/// One staged position for a body, and the branch condition it was staged under.
#[derive(Clone, Debug)]
struct Staging {
    /// The branch this staging happened on.
    gate: BranchGate,
    /// Where it left the body (snapped floor cell).
    pos: [i32; 3],
    /// The facing it left the body in, when the leg planned one.
    yaw: Option<i32>,
}

/// The staging history of every walked body, in campaign effect order.
type StagingHistory = BTreeMap<String, Vec<Staging>>;

/// The most recent staging of `body` that provably already happened on the branch
/// a leg gated by `gate` runs on — the origin that leg's walk must start from.
///
/// Walks the history backwards and takes the first entry whose own gate is
/// implied by `gate`. `None` means nothing in the history is provable on this
/// branch, and the caller falls back to the body's declared home anchor — the
/// pre-chaining behaviour, which is right precisely when no prior leg is proven.
fn chained_staging<'a>(
    history: &'a StagingHistory,
    body: &str,
    gate: &BranchGate,
) -> Option<&'a Staging> {
    history
        .get(body)?
        .iter()
        .rev()
        .find(|s| s.gate.implied_by(gate))
}

/// Record where a leg left a body, on the branch it ran on.
fn record_staging(
    history: &mut StagingHistory,
    body: &str,
    gate: BranchGate,
    pos: [i32; 3],
    yaw: Option<i32>,
) {
    history
        .entry(body.to_string())
        .or_default()
        .push(Staging { gate, pos, yaw });
}

/// `DW0488` for a deduped occurrence whose branch-correct origin is not the one
/// the shared driver was planned from.
fn shared_origin_error(
    verb: &str,
    body: &str,
    to_anchor: &str,
    planned_from: [i32; 3],
    planned_gate: &BranchGate,
    actual_from: [i32; 3],
    this_gate: &BranchGate,
) -> Failure {
    let describe = |g: &BranchGate| {
        if g.is_unconditional() {
            "unconditionally".to_string()
        } else {
            let mut parts = Vec::new();
            if !g.requires.is_empty() {
                parts.push(format!(
                    "requires {}",
                    g.requires.iter().cloned().collect::<Vec<_>>().join(", ")
                ));
            }
            if !g.forbids.is_empty() {
                parts.push(format!(
                    "forbids {}",
                    g.forbids.iter().cloned().collect::<Vec<_>>().join(", ")
                ));
            }
            format!("when it {}", parts.join(" and "))
        }
    };
    Failure {
        code: DW_MOVE_ORIGIN_SHARED,
        message: format!(
            "{verb}: `{body}` walks to `{to_anchor}` from two different places, but both beats \
             share ONE emitted walk driver, so one of them opens by teleporting the body across \
             the map. The driver is planned from {planned_from:?} (the occurrence that fires \
             {}), while the occurrence that fires {} leaves the body at {actual_from:?}. A walk \
             driver is content-keyed by `(body, destination)`, so it can carry only one origin. \
             Prescription: give the two beats distinct destinations (a second anchor a step apart \
             reads identically in play), or walk the body to a shared staging mark first so both \
             occurrences start from the same cell",
            describe(planned_gate),
            describe(this_gate),
        ),
    }
}

/// Default NPC walking speed in blocks/tick (spec-0008 §5; owner spike). Used when
/// a `move-npc` effect omits `speed`.
pub const DEFAULT_SPEED: f64 = 0.15;

/// The largest horizontal fraction of a one-cell crossing a body of hitbox
/// `width` may travel before its AABB reaches the boundary of the cell it is
/// entering — and therefore the whole horizontal budget a step-up has to gain
/// its block in, and a step-down has to lose one in.
///
/// Cell centres are 1 apart and the shared face is at 0.5; an AABB of
/// half-width `w/2` touches it at `0.5 - w/2`. A body 1.0 wide or wider has no
/// budget at all and gets the old L, which is not a special case to code around:
/// such a body needs more than one column ([`Footprint::for_dims`]) and could
/// not have been standing beside a full step block in the first place.
///
/// **The width is the body that SHIPS, never the footprint that routed.**
/// `move-npc` deliberately plans on the player footprint whatever the NPC wears,
/// and [`crate::compiler::clearance`] judges the body vanilla actually summons — so a
/// sheep-bodied NPC given the player's 0.2 budget is carried a fifth of the way
/// across while still below its landing and ends up inside the step block. The
/// routed footprint bounds *where* the body may go; the rendered body bounds
/// *how fast it may get there*, and `DW0450` is the check that says so.
fn step_fold(width: f64) -> f64 {
    (0.5 - width / 2.0).max(0.0)
}

/// The intermediate waypoints a one-cell vertical step inserts between the two
/// cell centres `a` (source) and `b` (destination), for a body `width` × `height`.
///
/// **The defect this shape exists for (owner playtest, island staging round).**
/// The step-up used to insert a single vertex *directly above the source cell*:
/// the body rose a whole block with no horizontal motion at all and then crossed
/// level, which reads as an animal riding an invisible lift rather than hopping
/// a ledge — the same walk that reddened `DW0453`. The route model calls that
/// move a **jump** (it charges it a jump arc and demands jump head clearance),
/// so the rendered motion was not the move the plan modelled.
///
/// The shape now emitted, and the one number in it is derived rather than
/// picked:
///
/// * **Step up** — rise to the destination surface over the first
///   [`step_fold`] of the crossing, so the body is already moving forward while
///   it gains the block and the rise completes exactly as its AABB reaches the
///   step block's face.
/// * **Step down** — hold the source height until the body's AABB has cleared
///   the cell it is leaving (the last [`step_fold`] of the crossing), then drop
///   into the destination while still moving forward.
///
/// Every emitted point stays inside cells `neighbors_fp` proved clear, which is
/// what the old L bought and is not given up here. The emitted y-range is the
/// L's exactly — only *when* the height changes moves — which is what makes the
/// new shape provably no worse than the shipped one.
///
/// **What is deliberately NOT emitted, and why it is a gap in the PROOF rather
/// than in the rendering.** A body that hops a ledge goes *above* it and comes
/// down; that reads as a hop, and it is what the route model itself says the
/// move is (it charges a jump arc and, past the auto-step budget, demands jump
/// head clearance). No such arc is soundly renderable today, and it was measured
/// rather than reasoned about: an apex bounded by the slack between the ROUTED
/// footprint and its whole cells put a villager's real 1.95-tall body into a
/// solid block, because `move-npc` plans on the player's 1.8 while
/// [`crate::compiler::clearance`] judges the body that ships. Sizing it against the true
/// body does not rescue it either: at the apex the body straddles both columns,
/// and the course it reaches over the SOURCE column is cleared only by
/// `head_clear_to_jump` — which [`World::neighbors_fp`] demands only when the
/// rise is past the auto-step budget, so a cell-level `+1` off a bottom slab is
/// a walk-up and nothing above the source was ever asked about. **The route
/// proof clears the step's endpoints and, for a jump, one head cell; it does not
/// clear the volume an arc sweeps.** Until it does, the rise folds into the
/// crossing and stops there.
fn step_vertices(a: [f64; 3], b: [f64; 3], width: f64) -> Vec<[f64; 3]> {
    let (dx, dz) = (b[0] - a[0], b[2] - a[2]);
    let lerp = |f: f64, y: f64| [a[0] + dx * f, y, a[2] + dz * f];
    let fold = step_fold(width);
    match (b[1] - a[1]).round() as i32 {
        1 => vec![lerp(fold, b[1])],
        -1 => vec![lerp(1.0 - fold, a[1])],
        _ => Vec::new(),
    }
}

/// String-pull an A* cell route into the polyline a body actually walks: wherever
/// the straight segment between two cells of the route is walkable in its own
/// right ([`World::segment_walkable_fp`]), the cells between them are dropped.
///
/// **The defect this exists for (owner playtest, castle-tour staging).** A walk is
/// planned by A* over a four-connected grid, so every route is a staircase of
/// axis-aligned segments. At room scale the staircase is invisible; crossing a
/// courtyard or a wall-walk it is a body zig-zagging along gridlines, which reads
/// as a machine tracing a floor plan rather than a person walking across a yard.
///
/// **What is smoothed, and what is deliberately not.** A run is merged only while
/// it stays level and unobstructed for the body's real width — every clause is in
/// [`World::segment_walkable_fp`], which is the same standability rule A* itself
/// stepped with, so a smoothed segment is proven by the model that proved the
/// route rather than by a second, looser one. Every rise, drop, slab lip, stair
/// edge and use-gate therefore ENDS a run and survives into the walked polyline as
/// its own vertex, where [`resample_body`]'s cardinal step shape renders it
/// exactly as before. **A route with no level run of three or more cells comes back
/// unchanged**, which is why a short indoor walk emits the bytes it always did.
///
/// The scan is the classic greedy string-pull: from each kept vertex, extend while
/// the straight line still clears, stop at the first cell it does not, keep that
/// one and start again. Stopping at the first failure rather than hunting further
/// along the route is what makes it O(route × run) instead of quadratic, and it
/// cannot skip a corner — a corner is exactly where the line stops clearing.
///
/// Determinism (ADR-0006): a pure function of the route and the world, with a
/// fixed scan order and integer-only tests.
///
/// **The route itself is not touched.** [`MovePlan::cells`] keeps the full A* cell
/// path, because the traversal proof ([`crate::compiler::traversal`]) asks of it
/// what move the body made — which cell it entered, which it stepped up onto, and
/// which use-gate it passed — and a thinned route would silently stop binding
/// those rules. This is the polyline the body is RENDERED along; the proof keeps
/// the cells.
fn smooth_walk(world: &World, cells: &[[i32; 3]], fp: &Footprint, width: f64) -> Vec<[i32; 3]> {
    if cells.len() < 3 {
        return cells.to_vec();
    }
    let mut out = vec![cells[0]];
    let mut i = 0;
    while i + 1 < cells.len() {
        // The furthest cell the straight line from `cells[i]` still reaches.
        // Never less than `i + 1`, so the scan always advances and the walked
        // polyline is never longer than the route.
        let mut far = i + 1;
        for j in (i + 2)..cells.len() {
            if !world.segment_walkable_fp(cells[i], cells[j], fp, width) {
                break;
            }
            far = j;
        }
        out.push(cells[far]);
        i = far;
    }
    out
}

/// [`resample`] for a body of the given hitbox — the footprint the leg was
/// **routed** under, since the rendered motion is bounded by the volume the
/// proof proved and a body the router never saw was never proved anything.
///
/// Returns the emitted samples and the same samples **before** the 0.01-block
/// rounding, in that order.
///
/// The emitted `tp` coordinates are rounded so they stay short and byte-stable.
/// That rounding is invisible to a body walking a cardinal path — every step is
/// axis-aligned, so the bearing between two rounded samples is the same cardinal
/// bearing as between two exact ones. It is NOT invisible to a body walking a
/// diagonal: at 0.15 blocks a tick, a ±0.005 wobble on each component is up to a
/// couple of degrees of noise, so [`yaws_along`] reading the rounded samples
/// gives a body crossing a courtyard in a straight line a yaw that twitches every
/// single tick — an emitted jitter far more visible than the right angles the
/// straight line was cut to remove. The yaw is a property of the segment being
/// walked, so it is taken from the exact samples; only the position is rounded.
fn resample_body(cells: &[[i32; 3]], speed: f64, width: f64) -> (Vec<[f64; 3]>, Vec<[f64; 3]>) {
    let mut pts: Vec<[f64; 3]> = Vec::with_capacity(cells.len() * 3);
    for (i, c) in cells.iter().enumerate() {
        let p = cell_center(*c);
        if i > 0 {
            pts.extend(step_vertices(cell_center(cells[i - 1]), p, width));
        }
        pts.push(p);
    }
    if pts.len() == 1 {
        return (vec![pts[0]], vec![pts[0]]);
    }
    // Cumulative arc length at each vertex.
    let mut cum = vec![0.0f64];
    for w in pts.windows(2) {
        let d = ((w[1][0] - w[0][0]).powi(2)
            + (w[1][1] - w[0][1]).powi(2)
            + (w[1][2] - w[0][2]).powi(2))
        .sqrt();
        cum.push(cum.last().unwrap() + d);
    }
    let total = *cum.last().unwrap();
    let speed = if speed > 0.0 { speed } else { DEFAULT_SPEED };
    let ticks = ((total / speed).ceil() as i64).max(1) as usize;
    let mut out = Vec::with_capacity(ticks + 1);
    let mut exact = Vec::with_capacity(ticks + 1);
    for t in 0..=ticks {
        let d = total * (t as f64) / (ticks as f64);
        let p = point_at(&pts, &cum, d);
        exact.push(p);
        // Round to 0.01 block — far finer than needed at 0.15 blk/tick, and keeps
        // the emitted per-tick `tp` coordinates short and stable.
        out.push([round2(p[0]), round2(p[1]), round2(p[2])]);
    }
    *out.last_mut().unwrap() = *pts.last().unwrap();
    *exact.last_mut().unwrap() = *pts.last().unwrap();
    (out, exact)
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// The point at arc length `d` along the polyline `pts` with cumulative lengths
/// `cum`.
fn point_at(pts: &[[f64; 3]], cum: &[f64], d: f64) -> [f64; 3] {
    let total = *cum.last().unwrap();
    if d <= 0.0 || total == 0.0 {
        return pts[0];
    }
    if d >= total {
        return *pts.last().unwrap();
    }
    // Find the segment containing `d`.
    let mut i = 0;
    while i + 1 < cum.len() && cum[i + 1] < d {
        i += 1;
    }
    let seg = cum[i + 1] - cum[i];
    let f = if seg > 0.0 { (d - cum[i]) / seg } else { 0.0 };
    let a = pts[i];
    let b = pts[i + 1];
    [
        a[0] + (b[0] - a[0]) * f,
        a[1] + (b[1] - a[1]) * f,
        a[2] + (b[2] - a[2]) * f,
    ]
}

/// Give a walked body its arrival turn: rewrite the LAST entry of `yaws`, which
/// [`yaws_along`] left carrying the bearing of the final step.
///
/// **The rule belongs to the walked body, not to the verb that first needed it.**
/// The path's own tangent is the WRONG answer at the end of any walk: a body that
/// walked away from the party arrives with its back to them, which is what a
/// guide leading a tour must never do — and a `move-actor` puppet is as much a
/// body as a `move-npc` villager is. So the arrival yaw is the destination
/// anchor's **declared facing** when the piece declares one — the anchor is
/// where the piece says a body at that spot looks — and otherwise the reverse of
/// the last leg, which turns the body back the way it came, where whoever
/// followed it is standing.
///
/// `declared` is handed in rather than looked up here, because only the caller
/// knows the scope its mover's anchors resolve in: an NPC's are its own area's
/// ([`anchor_facing_yaw`]), an actor's are global ([`actor_anchor_facing_yaw`]).
fn apply_arrival_yaw(yaws: &mut [i32], declared: Option<i32>) {
    if let Some(last) = yaws.last_mut() {
        *last = arrival_yaw_of(declared, *last);
    }
}

/// The arithmetic half of [`apply_arrival_yaw`], so both branches are testable
/// without a whole `Plan`: a declared facing wins, and an undeclared one turns
/// the body through 180 degrees to face back down the path it just walked.
fn arrival_yaw_of(declared: Option<i32>, walk_yaw: i32) -> i32 {
    declared.unwrap_or_else(|| (walk_yaw + 180).rem_euclid(360))
}

/// The MC yaw (degrees, 0 = +z/south) for a horizontal movement delta, or `None`
/// for no horizontal motion. `yaw = atan2(-dx, dz)`.
fn yaw_of(dx: f64, dz: f64) -> Option<i32> {
    if dx.abs() < 1e-6 && dz.abs() < 1e-6 {
        return None;
    }
    let deg = (-dx).atan2(dz).to_degrees();
    let mut y = deg.round() as i32 % 360;
    if y < 0 {
        y += 360;
    }
    Some(y)
}

/// A yaw per waypoint, each the **exact bearing of the segment about to be
/// walked** (no smoothing: a corner turns on the tick it is taken); the last
/// reuses the previous. A body tp'd without a matching yaw moonwalks — shown by
/// packet evidence for puppets and visible in play for NPCs.
///
/// `seed` is the facing the body already has, used for any leading waypoints with
/// no horizontal motion of their own (a walk that opens with `resample`'s vertical
/// step-up leg, or a degenerate zero-length move). An established facing is never
/// overwritten with a fabricated south.
fn yaws_along(waypoints: &[[f64; 3]], seed: i32) -> Vec<i32> {
    let n = waypoints.len();
    let mut yaws = vec![0i32; n];
    // Forward pass: each waypoint faces its NEXT step; the final waypoint reuses the
    // last motion direction (so arrival keeps the walk facing, not a snap to south).
    let mut last = seed;
    for i in 0..n {
        if i + 1 < n {
            let a = waypoints[i];
            let b = waypoints[i + 1];
            if let Some(y) = yaw_of(b[0] - a[0], b[2] - a[2]) {
                last = y;
            }
        }
        yaws[i] = last;
    }
    yaws
}

/// The first cell along the straight start→target line the actor's footprint cannot
/// stand on — a best-effort "first blocked cell" for the `DW0325` message.
fn first_blocked_fp(world: &World, start: [i32; 3], target: [i32; 3], fp: &Footprint) -> [i32; 3] {
    let d = [
        target[0] - start[0],
        target[1] - start[1],
        target[2] - start[2],
    ];
    let steps = d[0].abs().max(d[1].abs()).max(d[2].abs()).max(1);
    for s in 0..=steps {
        let cell = [
            start[0] + d[0] * s / steps,
            start[1] + d[1] * s / steps,
            start[2] + d[2] * s / steps,
        ];
        if !world.standable_fp(cell, fp) {
            return cell;
        }
    }
    target
}

/// The world cells a timeline's sealed gate regions fill (see [`crate::compiler::timeline`]).
fn seal_cells(seal: &crate::compiler::timeline::GateState) -> BTreeSet<[i32; 3]> {
    let mut cells = BTreeSet::new();
    for &(lo, hi) in seal.keys() {
        cells.extend(crate::compiler::assembled::region_cells(lo, hi));
    }
    cells
}

/// Memoized timeline-sealed views of the world.
///
/// A staged walk is routed over the world with the gates its own timeline has
/// already shut forced solid ([`World::with_sealed`]). Building that view clones
/// the whole occupancy model, and a campaign typically has many walks sharing the
/// same handful of gate states, so views are cached by their region set. Keyed by
/// a sorted `Vec<Region>` and stored in insertion order: deterministic, no
/// hash-order iteration (ADR-0006).
#[derive(Default)]
struct SealCache {
    index: BTreeMap<Vec<crate::compiler::timeline::Region>, usize>,
    worlds: Vec<World>,
}

impl SealCache {
    /// The index of the sealed view for `seal`, or `None` when nothing is sealed
    /// (the caller then uses the base world — which is what keeps a campaign with
    /// no `close-gate` byte-identical: no clone, no different world, same routes).
    fn index_of(
        &mut self,
        base: &World,
        seal: &crate::compiler::timeline::GateState,
    ) -> Option<usize> {
        if seal.is_empty() {
            return None;
        }
        let key: Vec<crate::compiler::timeline::Region> = seal.keys().copied().collect();
        if let Some(&i) = self.index.get(&key) {
            return Some(i);
        }
        self.worlds.push(base.with_sealed(&seal_cells(seal)));
        let i = self.worlds.len() - 1;
        self.index.insert(key, i);
        Some(i)
    }
}

/// The `DW0410` diagnostic for a staged walk the timeline's own `close-gate`
/// makes impossible: names the verb, the mover, the leg, and every gate anchor
/// sealed ahead of it, plus the three ways out.
fn gate_timeline_error(
    verb: &str,
    mover: &str,
    to_anchor: &str,
    start: [i32; 3],
    target: [i32; 3],
    seal: &crate::compiler::timeline::GateState,
) -> Failure {
    let gates: Vec<&str> = seal.values().map(|s| s.as_str()).collect();
    Failure {
        code: DW_GATE_TIMELINE,
        message: format!(
            "{verb}: `{mover}` cannot walk the leg {start:?} → `{to_anchor}` {target:?} — the \
             route exists on the open world, but an EARLIER effect in this same timeline sealed \
             gate {} with `close-gate`, and no route remains once it is shut. The walk would \
             step through solid blocks at runtime. Move the walk before the `close-gate` (a \
             lower `at_ticks` / earlier position in the bundle), reopen the gate with \
             `open-gate` before the walk, or route the walk to a destination reachable on the \
             sealed side",
            gates
                .iter()
                .map(|g| format!("`{g}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {

    /// **Owner playtest (castle tour): a walk across open ground read as a machine
    /// tracing gridlines rather than a person crossing a yard.**
    ///
    /// A* is four-connected, so the route it proves across an open plaza is a
    /// staircase. String-pulling drops every cell between the endpoints, because
    /// the straight line between them is walkable in its own right — so the body
    /// crosses on the line a person would take, yawing once and holding.
    #[test]
    fn a_straight_open_run_smooths_to_its_two_endpoints() {
        let world = floored(20, 20, 65, &[]);
        let fp = Footprint::player();
        let (start, goal) = ([1, 65, 1], [15, 65, 9]);
        let route = world
            .find_path(start, goal)
            .expect("an open plaza connects");
        // Without this the test could pass on a route that was already a line.
        assert!(
            route.len() > 2,
            "fixture is vacuous — A* returned no staircase to pull: {route:?}"
        );
        let line = smooth_walk(&world, &route, &fp, PLAYER_WIDTH);
        assert_eq!(line, vec![start, goal]);
        // The shorter route is also the one the body is teleported along, and no
        // waypoint on it puts any part of the body inside a block.
        let before = resample_body(&route, DEFAULT_SPEED, PLAYER_WIDTH).0;
        let after = resample_body(&line, DEFAULT_SPEED, PLAYER_WIDTH).0;
        assert!(
            after.len() < before.len(),
            "smoothed route is not shorter: {} vs {}",
            after.len(),
            before.len()
        );
        for p in after {
            assert!(!aabb_clips(&world, p, PLAYER_WIDTH), "clips at {p:?}");
        }
    }

    /// A straight run yaws **once and holds** — which is half the point of walking
    /// it straight, and the half the 0.01-block coordinate rounding silently took
    /// away: on a diagonal, two rounded samples 0.15 apart differ in bearing by a
    /// degree or two, so a body crossing a courtyard in a perfectly straight line
    /// twitched its head every tick. The yaw belongs to the segment being walked,
    /// so it is read off the exact samples and the emitted position off the
    /// rounded ones.
    #[test]
    fn a_straight_diagonal_run_yaws_once_and_holds() {
        let world = floored(20, 20, 65, &[]);
        let fp = Footprint::player();
        let (start, goal) = ([1, 65, 1], [15, 65, 9]);
        let route = world
            .find_path(start, goal)
            .expect("an open plaza connects");
        let line = smooth_walk(&world, &route, &fp, PLAYER_WIDTH);
        let (rounded, exact) = resample_body(&line, DEFAULT_SPEED, PLAYER_WIDTH);
        let yaws = yaws_along(&exact, 0);
        assert!(yaws.len() > 50, "fixture too short to show a twitch");
        let distinct: BTreeSet<i32> = yaws.iter().copied().collect();
        assert_eq!(
            distinct.len(),
            1,
            "a straight run must hold one bearing, got {distinct:?}"
        );
        // And the defect this guards is real: the same yaw taken off the rounded
        // samples is not one bearing but many.
        let from_rounded: BTreeSet<i32> = yaws_along(&rounded, 0).into_iter().collect();
        assert!(
            from_rounded.len() > 1,
            "rounding no longer perturbs the bearing — this test now proves nothing"
        );
    }

    /// A wall between the endpoints is never smoothed through. The run stops at the
    /// corner it has to turn, every kept segment is walkable on its own — the
    /// invariant the whole pass rests on — and the body the emitter teleports stays
    /// out of the geometry.
    #[test]
    fn a_walk_around_a_wall_is_not_smoothed_through_it() {
        // A wall at x = 5 closing z = 0..=7 of an 11x11 room: the only way across is
        // round its end at z >= 8.
        let mut wall = Vec::new();
        for z in 0..8 {
            for dy in 0..2 {
                wall.push([5, 65 + dy, z]);
            }
        }
        let world = floored(11, 11, 65, &wall);
        let fp = Footprint::player();
        let (start, goal) = ([1, 65, 1], [9, 65, 1]);
        let route = world
            .find_path(start, goal)
            .expect("the wall has an end to round");
        let line = smooth_walk(&world, &route, &fp, PLAYER_WIDTH);
        // The straight line between the endpoints crosses the wall, and the swept
        // test is what refuses it.
        assert!(!world.segment_walkable_fp(start, goal, &fp, PLAYER_WIDTH));
        assert!(line.len() > 2, "the wall was smoothed away: {line:?}");
        // Every kept segment clears on its own — nothing was merged across geometry.
        // The fixture is level throughout, so this holds with no exemption.
        for pair in line.windows(2) {
            assert!(
                world.segment_walkable_fp(pair[0], pair[1], &fp, PLAYER_WIDTH),
                "kept an unwalkable segment {:?} -> {:?}",
                pair[0],
                pair[1]
            );
        }
        for p in resample_body(&line, DEFAULT_SPEED, PLAYER_WIDTH).0 {
            assert!(!aabb_clips(&world, p, PLAYER_WIDTH), "clips at {p:?}");
        }
    }

    /// The stated limit, bound: smoothing is level-only, so a height change cuts the
    /// run and survives as its own one-cell step. That is what keeps [`step_vertices`]
    /// rendering every rise the way it always did, and what stops a diagonal sliding
    /// a body over a stair's edge.
    #[test]
    fn a_step_ends_a_smoothed_run() {
        // A 12x3 floor whose far half stands one block higher: a single step at x = 6,
        // under a ceiling high enough that the step is a legal jump.
        let mut solid = BTreeSet::new();
        for x in 0..12 {
            for z in 0..3 {
                let top = if x < 6 { 64 } else { 65 };
                for y in 60..=top {
                    solid.insert([x, y, z]);
                }
                solid.insert([x, 70, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        let fp = Footprint::player();
        let route = world
            .find_path([0, 65, 1], [11, 66, 1])
            .expect("one step up connects the halves");
        let line = smooth_walk(&world, &route, &fp, PLAYER_WIDTH);
        assert!(
            line.windows(2).any(|w| w[0][1] != w[1][1]),
            "the step vanished into a diagonal: {line:?}"
        );
        for w in line.windows(2) {
            if w[0][1] != w[1][1] {
                let d = (w[0][0] - w[1][0]).abs() + (w[0][2] - w[1][2]).abs();
                assert_eq!(d, 1, "a height change must stay a one-cell step: {w:?}");
            }
        }
        for p in resample_body(&line, DEFAULT_SPEED, PLAYER_WIDTH).0 {
            assert!(!aabb_clips(&world, p, PLAYER_WIDTH), "clips at {p:?}");
        }
    }

    /// The step shape is a property of **every** body the emitter can move, not of
    /// the one hitbox a fixture happens to use, so it is swept over the closed
    /// space of single-column hitboxes rather than over a hand-written roster of
    /// mob ids (a roster goes stale the moment the dims table gains a row).
    ///
    /// Two assertions. No emitted segment is a vertical translation in place — for
    /// every body, in every direction. And, for a rise or a flat crossing, every
    /// waypoint's whole body volume stays inside the courses `neighbors_fp`
    /// actually proved, which is what bounds the hop's apex.
    ///
    /// **A one-block DROP is deliberately not held to the second assertion, and
    /// the reason is a gap in the proof rather than in the rendering.** A drop
    /// demands no jump headroom, so the destination column is cleared only over
    /// its own standing cells; a body crossing the shared face at the *source*
    /// height therefore occupies a course above the destination that nothing
    /// cleared, and no rendered path avoids it — the body cannot descend while it
    /// still stands over the cell it is leaving, and it overlaps the destination
    /// column before it has left. This shape narrows that overlap to the width of
    /// the body (the old one carried the source height all the way to the
    /// destination's centre) but cannot close it.
    #[test]
    fn no_walked_step_renders_a_vertical_translation_in_place() {
        let y = 64i64;
        let mut cases = 0usize;
        let mut vertical_segments = 0usize;
        let mut courses_checked = 0usize;
        let mut w100 = 5u32;
        while w100 < 100 {
            let width = f64::from(w100) / 100.0;
            let mut h20 = 10u32;
            while h20 <= 60 {
                let height = f64::from(h20) / 20.0;
                let tall = height.ceil() as i64;
                for rise in [-1i64, 0, 1] {
                    cases += 1;
                    let src = [0, y as i32, 0];
                    let dst = [1, (y + rise) as i32, 0];
                    let pts = resample_body(&[src, dst], DEFAULT_SPEED, width).0;
                    // (1) The SHAPE: no leg of the emitted polyline changes height
                    // without advancing. This is where the old L's defect lived —
                    // its first leg had a horizontal length of exactly zero.
                    let a = cell_center(src);
                    let b = cell_center(dst);
                    let mut poly = vec![a];
                    poly.extend(step_vertices(a, b, width));
                    poly.push(b);
                    for w in poly.windows(2) {
                        let dy = (w[1][1] - w[0][1]).abs();
                        let dh = ((w[1][0] - w[0][0]).powi(2) + (w[1][2] - w[0][2]).powi(2)).sqrt();
                        if dy > 1e-9 {
                            vertical_segments += 1;
                            assert!(
                                dh > 1e-9,
                                "w={width} h={height} rise={rise}: a leg changes height \
                                 without advancing, {:?} -> {:?}",
                                w[0],
                                w[1]
                            );
                        }
                    }
                    // (2) The emitted POSITIONS: the body is never carried up over
                    // the cell it is standing on, nor set down over the cell it is
                    // arriving at. Stated over positions rather than per-tick
                    // deltas because `round2` quantises x to 0.01, and a 0.9-wide
                    // body's entire horizontal budget for a step is 0.05 blocks —
                    // so single ticks inside the rise round to the same x and say
                    // nothing about whether the body is in place.
                    let (pivot, surface) = if rise > 0 { (a, b[1]) } else { (b, a[1]) };
                    for q in &pts {
                        let off = ((q[0] - pivot[0]).powi(2) + (q[2] - pivot[2]).powi(2)).sqrt();
                        assert!(
                            off > 1e-9 || (q[1] - surface).abs() > 1e-9 || rise == 0,
                            "w={width} h={height} rise={rise}: waypoint {q:?} reaches the \
                             far surface while still over {pivot:?} — a translation in place"
                        );
                    }
                    if rise < 0 {
                        continue; // see the doc comment: the proof, not the render
                    }
                    // The courses `neighbors_fp` proves, per column: the source's
                    // own feet cell plus `tall` above it (`standable_fp` +
                    // `head_clear_to_jump`), and the destination's `tall` cells.
                    let proven = |col: i32| -> (i64, i64) {
                        if col == src[0] {
                            (y, y + tall)
                        } else {
                            let d = i64::from(dst[1]);
                            (d, d + tall - 1)
                        }
                    };
                    for p in &pts {
                        let lo_col = (p[0] - width / 2.0).floor() as i32;
                        let hi_col = (p[0] + width / 2.0 - 1e-9).floor() as i32;
                        let lo_y = p[1].floor() as i64;
                        let hi_y = (p[1] + height - 1e-9).floor() as i64;
                        for col in lo_col..=hi_col {
                            let (plo, phi) = proven(col);
                            courses_checked += 1;
                            assert!(
                                lo_y >= plo && hi_y <= phi,
                                "w={width} h={height} rise={rise}: waypoint {p:?} occupies \
                                 courses {lo_y}..={hi_y} of column {col}, outside the proven \
                                 {plo}..={phi}"
                            );
                        }
                    }
                }
                h20 += 1;
            }
            w100 += 5;
        }
        // Stated, computed, non-vacuous: a sweep whose bounds had silently shrunk,
        // or one in which nothing ever changed height, would satisfy every
        // assertion above and prove nothing.
        assert_eq!(cases, 19 * 51 * 3);
        assert!(
            vertical_segments > 0,
            "the sweep ran {cases} cases and not one segment changed height"
        );
        assert!(courses_checked > cases);
    }

    #[test]
    fn resample_honors_speed_and_lands_exactly_on_target() {
        let cells = [[0, 65, 0], [10, 65, 0]];
        let slow = resample_body(&cells, 0.15, PLAYER_WIDTH).0;
        let fast = resample_body(&cells, 1.0, PLAYER_WIDTH).0;
        // Slower speed → more per-tick waypoints for the same distance.
        assert!(slow.len() > fast.len());
        // Endpoints are the CENTRES of the start/goal cells, not their corners:
        // a body positioned on the integer cell coordinate straddles four columns.
        assert_eq!(*slow.last().unwrap(), cell_center([10, 65, 0]));
        assert_eq!(slow[0], cell_center([0, 65, 0]));
    }

    #[test]
    fn yaw_follows_the_movement_tangent() {
        // MC yaw: 0 = +z (south), 90 = -x (west), 180 = -z (north), 270 = +x (east).
        assert_eq!(yaw_of(0.0, 1.0), Some(0));
        assert_eq!(yaw_of(-1.0, 0.0), Some(90));
        assert_eq!(yaw_of(0.0, -1.0), Some(180));
        assert_eq!(yaw_of(1.0, 0.0), Some(270));
        assert_eq!(yaw_of(0.0, 0.0), None);
        // A straight +x path yaws every waypoint east (270), including the last.
        let wps = vec![[0.0, 65.0, 0.0], [1.0, 65.0, 0.0], [2.0, 65.0, 0.0]];
        assert_eq!(yaws_along(&wps, 0), vec![270, 270, 270]);
    }

    /// A walk does not end facing the way it was going. The destination anchor's
    /// declared facing wins, and an anchor that declares none turns the body back
    /// the way it came — where whoever followed it is standing.
    #[test]
    fn a_walk_ends_facing_back_down_the_path_it_walked() {
        // walked north (180): arrive facing south (0), back toward the follower
        assert_eq!(arrival_yaw_of(None, 180), 0);
        // walked south (0): arrive facing north (180)
        assert_eq!(arrival_yaw_of(None, 0), 180);
        // walked east (270): arrive facing west (90)
        assert_eq!(arrival_yaw_of(None, 270), 90);
        // and a declared facing is the piece's word on where a body there looks
        assert_eq!(arrival_yaw_of(Some(270), 180), 270);
        assert_eq!(arrival_yaw_of(Some(0), 0), 0);
    }

    /// The corner turns on the tick it is taken: each waypoint carries the exact
    /// bearing of the segment it is about to walk, with no smoothing between the
    /// two legs. What the ARRIVAL waypoint carries is `arrival_yaw`'s business,
    /// and is asserted above.
    #[test]
    fn yaw_turns_at_a_direction_change() {
        // +x for two steps (east, 270), then +z for two (south, 0).
        let wps = vec![
            [0.0, 65.0, 0.0],
            [1.0, 65.0, 0.0],
            [2.0, 65.0, 0.0],
            [2.0, 65.0, 1.0],
            [2.0, 65.0, 2.0],
        ];
        assert_eq!(yaws_along(&wps, 180), vec![270, 270, 0, 0, 0]);
    }

    /// A leading segment with no horizontal motion (`resample`'s vertical step-up
    /// leg) keeps the seed — the facing the body already has — instead of
    /// fabricating a snap to south.
    #[test]
    fn yaw_keeps_the_seed_until_the_first_horizontal_step() {
        // Rise in place, then walk -z (north, 180).
        let wps = vec![
            [0.5, 65.0, 0.5],
            [0.5, 66.0, 0.5],
            [0.5, 66.0, -0.5],
            [0.5, 66.0, -1.5],
        ];
        assert_eq!(yaws_along(&wps, 90), vec![90, 180, 180, 180]);
        // A degenerate zero-length move never overrides the established facing.
        assert_eq!(yaws_along(&[[0.0, 65.0, 0.0]], 90), vec![90]);
    }
}
