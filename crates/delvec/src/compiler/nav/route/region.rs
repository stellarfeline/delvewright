//! What every runtime-written region has become at one point of the quest DAG
//! ([`RegionState`]), the world a leg is routed under, and the blame a refused
//! leg names.

use crate::compiler::nav::world::{
    LethalRegion, Liveness, StagedVolume, World, liveness_of, loop_gate_words,
};
use crate::compiler::plan::{Plan, RegionEvent, RegionEvents, RegionWrite};
use std::collections::{BTreeMap, BTreeSet};

/// What every runtime-written region has become, as of one point in the quest DAG:
/// the cells a write has made solid, and the cells a write has cleared.
///
/// One value, because it is one question. A proof that asked only "what is walled
/// off" is the shape the capability had while `close-gate` owned it privately, and
/// it is why the other half — "what is now open" — had no answer for anything but a
/// gate, whose cells the assembled model happens to hold open unconditionally.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(in crate::compiler::nav) struct RegionState {
    /// Cells a runtime fill has made solid by this point.
    pub(in crate::compiler::nav) solid: BTreeSet<[i32; 3]>,
    /// Cells a runtime clear has emptied by this point.
    pub(in crate::compiler::nav) cleared: BTreeSet<[i32; 3]>,
    /// Cells a runtime fill has filled with **fluid** by this point
    /// ([`crate::compiler::plan::RegionWrite::Flood`]) — impassable, and never floor.
    ///
    /// A third set rather than a flag on `solid`, because the two answers differ in
    /// the direction that matters: a body cannot walk through either, and can stand
    /// on only one. Folding a water fill into `solid` is what let the route proof
    /// walk a party across a pond in mid-air.
    pub(in crate::compiler::nav) flooded: BTreeSet<[i32; 3]>,
    /// The boxes behind `flooded`, in region order — carried so a route failure can
    /// NAME the write that caused it instead of reporting unroutable geometry that
    /// looks perfectly open, exactly as [`World::lethal_regions`] does for a lethal
    /// volume.
    pub(in crate::compiler::nav) flood_regions: Vec<([i32; 3], [i32; 3])>,
    /// Cells an **unforced** fill has written by this point: a solid block laid from
    /// a beat the party may never play ([`crate::compiler::plan::RegionEvent::is_forced`]).
    ///
    /// A fourth set for the same reason `flooded` is a third: the two answers differ
    /// in the direction that matters. The party may arrive to find this box walled,
    /// so the proof may not walk *through* it; the party may equally arrive to find
    /// it as the world built it, so the proof may not stand *on* it. That is the
    /// pointwise-worst of the two futures, and it is the only reading of an unforced
    /// fill that is sound in both.
    ///
    /// Folding it into `solid` is what let a forced leg cross a chasm on a plank a
    /// trapped chest lays — provably completable, and physically unwalkable for a
    /// party that never opened the chest.
    pub(in crate::compiler::nav) unforced: BTreeSet<[i32; 3]>,
    /// The boxes behind `unforced`, each with the beat that lays it in words —
    /// carried for the same reason `flood_regions` is, so a route failure can NAME
    /// the beat instead of reporting geometry that reads perfectly open.
    pub(in crate::compiler::nav) unforced_regions: Vec<UnforcedBox>,
    /// Cells a **holding loop's slab** covers on this leg (spec-0086 §5.1) —
    /// impassable and never floor, the unforced shape, kept apart so a route
    /// failure names the loop and the gate term that still holds it.
    pub(in crate::compiler::nav) held: BTreeSet<[i32; 3]>,
    /// The slabs behind `held`, each with its loop and gate in words.
    pub(in crate::compiler::nav) held_regions: Vec<UnforcedBox>,
    /// Cells of every **staged** lethal volume that may be live at this point
    /// (spec-0088, [`World::staged_liveness`]) — impassable, widened by the
    /// walker's body, never floor, exactly as a volume live from world-load.
    pub(in crate::compiler::nav) lethal: BTreeSet<[i32; 3]>,
    /// The volumes behind `lethal`, as `(id, box)`, in declaration order, so a
    /// refusal can name the volume.
    pub(in crate::compiler::nav) lethal_regions: Vec<LethalRegion>,
    /// Per region, the causally-latest **forced** write and the block it lays
    /// (`None` = air), in region order — what [`RegionState::blocks_over`] lays
    /// over the assembled bytes to give this configuration's block map.
    pub(in crate::compiler::nav) laid: Vec<LaidWrite>,
}

/// One region a forced write lays, with the block its command writes (`None` =
/// air) — what [`RegionState::blocks_over`] lays over the assembled bytes.
type LaidWrite = (([i32; 3], [i32; 3]), Option<String>);

/// One box an unforced fill writes, with the beat that lays it in words — the blame
/// unit [`DW_UNFORCED_FOOTING`] reports.
type UnforcedBox = (([i32; 3], [i32; 3]), String);

/// Per region, the causally-latest write that precedes a leg: its firing step, what
/// it leaves, whether the party is forced to cause it, and the beat to blame if they
/// are not. Forcedness travels WITH the winner, so latest-write-wins needs no special
/// case for a forced write landing on top of an unforced one.
type LatestWrite = (usize, RegionWrite, bool, String, Option<String>);

impl RegionState {
    /// Nothing has been written by this point — the caller routes the base world
    /// and clones nothing.
    pub(in crate::compiler::nav) fn is_empty(&self) -> bool {
        self.solid.is_empty()
            && self.cleared.is_empty()
            && self.flooded.is_empty()
            && self.unforced.is_empty()
            && self.held.is_empty()
            && self.lethal.is_empty()
    }

    /// This state with every holding loop released — the counterfactual a
    /// route failure is tested against to say a loop, and not the geometry,
    /// closed the leg (spec-0086 §5.1).
    pub(in crate::compiler::nav) fn released(&self) -> RegionState {
        let mut st = self.clone();
        st.held.clear();
        st.held_regions.clear();
        st
    }

    /// **This configuration's bytes** (spec-0088 §5): the assembled block map
    /// with every forced write this configuration credits laid as the block its
    /// emitted command writes — a fill's block, air for a clear or an unseal.
    /// An unforced write is not laid: a block a beat nobody has to play lays is
    /// no signal the party is shown. A write whose block the model does not know
    /// (a world-load seal: the bytes already hold it) leaves the bytes as they
    /// are.
    ///
    /// The one derivation of a configuration's block map, for whoever asks:
    /// `DW0891` reads it to decide whether a caught cell is shown here.
    pub(crate) fn blocks_over(
        &self,
        base: &crate::compiler::blockstate::BlockMap,
    ) -> crate::compiler::blockstate::BlockMap {
        let mut out = base.clone();
        for ((lo, hi), block) in &self.laid {
            for c in crate::compiler::assembled::region_cells(*lo, *hi) {
                match block {
                    Some(b) => {
                        out.insert(c, crate::compiler::blockstate::BlockState::new(b));
                    }
                    None => {
                        out.remove(&c);
                    }
                }
            }
        }
        out
    }

    /// This state as it would be **if every unforced fill were credited** — the
    /// counterfactual [`DW_UNFORCED_FOOTING`] is derived from, and exactly the model
    /// this compiler ran before a firing's forcedness reached the geometry.
    ///
    /// A leg that routes here and nowhere else failed *because* the only footing it
    /// had was laid by a beat nobody has to play.
    pub(in crate::compiler::nav) fn as_if_forced(&self) -> RegionState {
        let mut st = self.clone();
        st.solid.extend(st.unforced.iter().copied());
        st.unforced.clear();
        st
    }
}

/// The holding slabs a route's `cells` pass through (spec-0086 §5.1), each named
/// with its loop and the gate term that holds it.
pub(in crate::compiler::nav) fn held_blame_over(
    regions: &[UnforcedBox],
    cells: &[[i32; 3]],
) -> Vec<String> {
    let mut out: Vec<String> = regions
        .iter()
        .filter(|((lo, hi), _)| {
            cells
                .iter()
                .any(|c| (0..3).all(|i| lo[i].min(hi[i]) <= c[i] && c[i] <= lo[i].max(hi[i])))
        })
        .map(|(_, why)| why.clone())
        .collect();
    if out.is_empty() {
        out.push("a loop's slab while the loop holds".to_string());
    }
    out
}

/// The unforced boxes a route's `cells` stand in or on, each named with the beat that
/// lays it — the cell itself and the cell **below** it, because footing is what this
/// asks about, exactly as [`World::flood_regions_over`] does.
///
/// A free function over the ledger rather than a method on [`RegionState`], because
/// the caller has already handed the state's other halves to the route it proved and
/// must not be made to keep the whole value alive to say what went wrong.
pub(in crate::compiler::nav) fn unforced_blame_over(
    regions: &[UnforcedBox],
    cells: &[[i32; 3]],
) -> Vec<String> {
    let touched: Vec<[i32; 3]> = cells
        .iter()
        .flat_map(|c| [*c, [c[0], c[1] - 1, c[2]]])
        .collect();
    regions
        .iter()
        .filter(|((lo, hi), _)| {
            touched
                .iter()
                .any(|c| (0..3).all(|i| lo[i].min(hi[i]) <= c[i] && c[i] <= lo[i].max(hi[i])))
        })
        .map(|((lo, hi), why)| {
            format!(
                "[{}, {}, {}]..[{}, {}, {}] (laid by {why})",
                lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
            )
        })
        .collect()
}

/// The state of every runtime-written region on a walked leg arriving at the
/// objective at critical-path step `arrival` (DSL v0.6 `close-gate`, generalised in
/// v0.10 by spec-0031). A write counts only if its firing objective is a **causal
/// (DAG) ancestor** of the leg's objective — `ancestor(ev.fire_step, arrival)` —
/// i.e. it is guaranteed to have happened before this leg in *every* valid play
/// order. That excludes a write on a parallel quest branch that the lineariser
/// merely happens to interleave ahead of this leg (which would falsely seal it).
/// Among the causally-preceding writes on a region, the **latest** (max
/// `fire_step`, respecting the DAG linearisation) wins: the region is solid iff
/// that latest write is a fill, and cleared iff it is a clear.
///
/// A campaign that writes no region yields an empty state and routes byte-
/// identically to the base world.
///
/// **The world's own writes come first.** The list this reasons over is not the
/// campaign's alone: a gate the placed prefabs author shut is a `Fill` at step 0
/// ([`World::world_load_seals`]) and enters here exactly like a `close-gate`. It is
/// a method on [`World`] for that reason — the world is the only object that knows
/// what it was built holding, and routing it through here is what makes an
/// `open-gate` the thing that *opens* a door rather than the thing that *proves it
/// was a door*. Before that, a gate region was passable unless a `close-gate`
/// sealed it, so the one mistake an author actually makes — forgetting to open a
/// door — was the one mistake this model could not represent.
impl World {
    pub(in crate::compiler::nav) fn region_state_at(
        &self,
        region_events: &RegionEvents,
        arrival: usize,
        ancestor: &dyn Fn(usize, usize) -> bool,
    ) -> RegionState {
        self.region_state_inner(region_events, arrival, ancestor, true)
    }

    /// [`World::region_state_at`] as it would be **if the world had been built with
    /// every gate already open** — the counterfactual [`DW_GATE_NEVER_OPENED`] is
    /// derived from, and the exact model this compiler shipped before the seals
    /// were measured.
    fn region_state_without_world_load(
        &self,
        region_events: &RegionEvents,
        arrival: usize,
        ancestor: &dyn Fn(usize, usize) -> bool,
    ) -> RegionState {
        self.region_state_inner(region_events, arrival, ancestor, false)
    }

    fn region_state_inner(
        &self,
        region_events: &RegionEvents,
        arrival: usize,
        ancestor: &dyn Fn(usize, usize) -> bool,
        world_load: bool,
    ) -> RegionState {
        // Per region, the causally-latest write that precedes this leg (ancestor of
        // the arrival objective); higher `fire_step` overrides. The winner carries
        // its own forcedness and blame, so a forced write landing after an unforced
        // one on the same box restores ordinary footing by winning, with no special
        // case: latest-write-wins already says which firing the party will find.
        let mut latest: BTreeMap<([i32; 3], [i32; 3]), LatestWrite> = BTreeMap::new();
        let world_load: Vec<RegionEvent> = if world_load {
            // FORCED: a gate the placed prefabs author shut is shut because the world
            // was built that way, not because anyone played a beat.
            self.modelled_seals()
                .map(|s| RegionEvent::forced(s.region, RegionWrite::Fill, 0))
                .collect()
        } else {
            Vec::new()
        };
        for ev in world_load.into_iter().chain(region_events.iter().cloned()) {
            if ancestor(ev.fire_step, arrival) {
                let key = (
                    ev.fire_step,
                    ev.write,
                    ev.is_forced(),
                    ev.blame().to_string(),
                    ev.block().map(str::to_string),
                );
                let e = latest.entry(ev.region).or_insert_with(|| key.clone());
                if ev.fire_step >= e.0 {
                    *e = key;
                }
            }
        }
        let mut st = RegionState::default();
        for (region, (_, write, forced, blame, block)) in latest {
            // The bytes this configuration holds: a forced write lays its block
            // (or air); a fill whose block the model does not know — a world-load
            // seal — leaves the bytes as the prefab built them.
            if forced {
                match write {
                    RegionWrite::Clear | RegionWrite::Unseal => st.laid.push((region, None)),
                    RegionWrite::Fill | RegionWrite::Flood | RegionWrite::Pass => {
                        if block.is_some() {
                            st.laid.push((region, block));
                        }
                    }
                }
            }
            // An `Unseal` contributes to no set: it removes the gate's own block, and
            // the base world holds the gate cells empty. It matters above, in
            // latest-write-wins, where it is what cancels a fill — including the
            // world-load fill this gate was born with.
            //
            // A `Fill` splits on forcedness and nothing else does. `Flood` needs no
            // split (impassable and never floor is already the worst of both
            // futures); `Clear`, `Unseal` and `Pass` never reach here unforced, because
            // `plan::collect_region_events` drops them — an unforced firing may make
            // a region impassable and may never make one passable.
            let into = match write {
                RegionWrite::Fill if !forced => {
                    st.unforced_regions.push((region, blame));
                    &mut st.unforced
                }
                RegionWrite::Fill => &mut st.solid,
                // A pass leaves cells a body occupies: to the walk, a clear.
                RegionWrite::Clear | RegionWrite::Pass => &mut st.cleared,
                RegionWrite::Flood => {
                    st.flood_regions.push(region);
                    &mut st.flooded
                }
                RegionWrite::Unseal => continue,
            };
            into.extend(crate::compiler::assembled::region_cells(region.0, region.1));
        }
        // spec-0086 §5.1: every loop slab whose gate may be open here holds,
        // read through the same liveness a staged volume's gate is.
        for (v, live) in
            self.loop_slabs
                .iter()
                .zip(self.loop_liveness(region_events, arrival, ancestor))
        {
            if live.may {
                st.held.extend(crate::compiler::assembled::region_cells(
                    v.region.0, v.region.1,
                ));
                st.held_regions.push((
                    v.region,
                    format!(
                        "the slab of loop `{}` ([{}, {}, {}]..[{}, {}, {}]), which holds at \
                         critical-path step {arrival}: read there, its gate {} may be open",
                        v.id,
                        v.region.0[0],
                        v.region.0[1],
                        v.region.0[2],
                        v.region.1[0],
                        v.region.1[1],
                        v.region.1[2],
                        loop_gate_words(&v.gate)
                    ),
                ));
            }
        }
        // spec-0088: every staged lethal volume that may be live here.
        for (v, live) in
            self.staged_lethal
                .iter()
                .zip(self.staged_liveness(region_events, arrival, ancestor))
        {
            if live.may {
                st.lethal.extend(crate::compiler::assembled::region_cells(
                    v.region.0, v.region.1,
                ));
                st.lethal_regions.push((v.id.clone(), v.region));
            }
        }
        st
    }

    /// **Whether each loop's slab may hold, and does, at this arrival**
    /// (spec-0086 §5.1) — one [`Liveness`] per [`World::loop_slabs`] entry, in
    /// declaration order, through [`liveness_of`]: the one reading of a gate
    /// into a region state, shared with the staged lethal volumes.
    pub(crate) fn loop_liveness(
        &self,
        events: &RegionEvents,
        arrival: usize,
        ancestor: &dyn Fn(usize, usize) -> bool,
    ) -> Vec<Liveness> {
        self.loop_slabs
            .iter()
            .map(|v| liveness_of(&v.gate, events, arrival, ancestor))
            .collect()
    }

    /// **Which staged lethal volumes may be, and are, live at this arrival**
    /// (spec-0088 §4.1) — one [`Liveness`] per [`World::staged_lethal`] entry,
    /// in declaration order. The one place a volume's gate is turned into a
    /// lethal set; [`World::region_state_at`]'s derivation calls it and nothing
    /// else decides it.
    pub(crate) fn staged_liveness(
        &self,
        events: &RegionEvents,
        arrival: usize,
        ancestor: &dyn Fn(usize, usize) -> bool,
    ) -> Vec<Liveness> {
        self.staged_lethal
            .iter()
            .map(|v| liveness_of(&v.gate, events, arrival, ancestor))
            .collect()
    }

    /// The staged lethal volumes this world holds, in declaration order.
    pub(crate) fn staged_volumes(&self) -> &[StagedVolume] {
        &self.staged_lethal
    }

    /// For a refusal that names `ids`: the staged ones among them, each with its
    /// gate and its state at the configuration arriving at critical step
    /// `arrival` (spec-0088 §4.2) — empty when none is staged, so a message
    /// about a volume live from world-load reads as it always has.
    pub(crate) fn staged_words(
        &self,
        ids: &[&str],
        events: &RegionEvents,
        arrival: usize,
        ancestor: &dyn Fn(usize, usize) -> bool,
    ) -> String {
        let live = self.staged_liveness(events, arrival, ancestor);
        let named: Vec<String> = self
            .staged_lethal
            .iter()
            .zip(live)
            .filter(|(v, _)| ids.contains(&v.id.as_str()))
            .map(|(v, l)| {
                format!(
                    "`{}` is live from a story stage ({}) and {} in the configuration arriving at \
                     critical step {arrival}",
                    v.id,
                    v.gate.words(),
                    l.words()
                )
            })
            .collect();
        if named.is_empty() {
            String::new()
        } else {
            format!(" In this configuration: {}.", named.join("; "))
        }
    }

    /// The runtime-region state for the walked leg `from_step → to_step` — the
    /// single definition of "which regions are filled and which are cleared while
    /// the player walks this leg", shared by the completability proof, the forced-
    /// cell set the trap proof reasons about, and the exported harness
    /// waypoints.
    ///
    /// A write is credited when the party has **necessarily** fired it by the time
    /// it walks this leg: it precedes the arrival (`ancestor(g, to_step)`), or it
    /// is the start step's own firing, or it precedes the start
    /// (`ancestor(g, from_step)`). The party stands at the start having done the
    /// start and everything before it, and walks toward an arrival whose
    /// predecessors are done; nothing else is guaranteed. Latest-write-wins then
    /// runs over that set exactly as it does at an arrival.
    ///
    /// For a causal leg — the start is itself an ancestor of the arrival — the
    /// start and its ancestors are already the arrival's, so this is the arrival's
    /// state. A leg the ancestry does not connect (the lineariser concatenating two
    /// sibling branches, or an ordering the relation fails to record) is judged
    /// over the same rule, never over the open world: a close a sibling branch
    /// fires that neither end inherits does not seal it, while the world-load
    /// seals (step `0`) and every write the start's own branch made do.
    fn leg_region_state(
        &self,
        region_events: &RegionEvents,
        ancestor: &dyn Fn(usize, usize) -> bool,
        from_step: usize,
        to_step: usize,
    ) -> RegionState {
        let fired = leg_fired(ancestor, from_step, to_step);
        self.region_state_at(region_events, to_step, &fired)
    }

    /// The same leg with the world-load gate seals lifted — see
    /// [`World::region_state_without_world_load`].
    pub(in crate::compiler::nav) fn leg_region_state_without_world_load(
        &self,
        region_events: &RegionEvents,
        ancestor: &dyn Fn(usize, usize) -> bool,
        from_step: usize,
        to_step: usize,
    ) -> RegionState {
        let fired = leg_fired(ancestor, from_step, to_step);
        self.region_state_without_world_load(region_events, to_step, &fired)
    }

    /// [`World::leg_region_state`] for a leg the player is asked to WALK — the
    /// single site that decides it, shared by the proof (`route_visited`) and the
    /// exported routes (`route_walked_legs`), so the route the harness walks is the
    /// route the proof passed. A leg that starts inside a teleport's volume is
    /// judged like every other leg: where the teleport is a link, the leg is the
    /// link (spec-0083 §3.4); where it is a gather, the party walks.
    pub(in crate::compiler::nav) fn walked_leg_region_state(
        &self,
        region_events: &RegionEvents,
        ancestor: &dyn Fn(usize, usize) -> bool,
        from_step: usize,
        to_step: usize,
    ) -> RegionState {
        self.leg_region_state(region_events, ancestor, from_step, to_step)
    }
}

/// The "has the party fired step `g`" predicate for the walked leg
/// `from_step → to_step` ([`World::leg_region_state`]), in the
/// `(fire_step, arrival)` shape [`World::region_state_at`] asks it in. The
/// arrival argument is ignored: the leg's own two ends decide.
fn leg_fired(
    ancestor: &dyn Fn(usize, usize) -> bool,
    from_step: usize,
    to_step: usize,
) -> impl Fn(usize, usize) -> bool + '_ {
    move |g, _| ancestor(g, to_step) || g == from_step || ancestor(g, from_step)
}

/// Render the [`DW_GATE_NEVER_OPENED`] blame clause for the gates a counterfactual
/// route crosses: each gate's anchor, its region, what the world puts in it, and —
/// the part that turns a symptom into a repair — **what the campaign does to it**.
///
/// Three answers, and they are three different bugs: nothing opens it anywhere
/// (a missing `open-gate`), something opens it but only from a bundle the party is
/// not forced to fire (a shop purchase, a sprung trap, a shortcut taken from the
/// far side — [`crate::compiler::plan::collect_region_events`]'s rule is that an optional
/// firing may seal a region and may never open one), or something opens it at a
/// step that is not a causal ancestor of this leg (opened, but too late, or on a
/// parallel branch).
pub(in crate::compiler::nav) fn gate_blame(
    gates: &[&crate::compiler::assembled::GateSeal],
    region_events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
    arrival: usize,
) -> String {
    if gates.is_empty() {
        return "(none — the counterfactual route crosses no measured gate seal; this is a \
                compiler defect, escalate it)"
            .to_string();
    }
    gates
        .iter()
        .map(|g| {
            let openers: Vec<usize> = region_events
                .iter()
                .filter(|e| e.region == g.region && e.write != RegionWrite::Fill)
                .map(|e| e.fire_step)
                .collect();
            let fate = if openers.is_empty() {
                // Either the campaign declares no `open-gate` on this anchor at
                // all, or every one it declares hangs off an OPTIONAL bundle, or
                // off a beat this path never plays, or behind a gate this path
                // does not satisfy, and was never credited
                // (`plan::collect_region_events`: an unforced firing may seal a
                // region and may never open one). One sentence for all three,
                // because the repair is the same — put the opening on a beat the
                // party cannot skip on this path.
                "no firing the party is forced to make ever opens it (an `open-gate` in an \
                 optional bundle — a shop purchase, a sprung trap, a death beat, a shortcut \
                 taken from the far side, a beat of a branch this path does not take, a line \
                 whose gate does not hold where this path plays it — is not credited, by the \
                 same rule that keeps every shortcut gate sealed so the delve is finishable the \
                 long way)"
                    .to_string()
            } else if openers.iter().any(|&s| ancestor(s, arrival)) {
                // Unreachable while this is the blamed gate — an opener that is a
                // causal ancestor cancels the world-load fill — and stated anyway,
                // because a silent third branch is how a blame list starts lying.
                "an opener DOES precede this leg (compiler defect, escalate it)".to_string()
            } else {
                let steps: Vec<String> = openers.iter().map(|s| s.to_string()).collect();
                format!(
                    "it is opened only at critical-path step(s) {} — after this leg, or on a \
                     branch the party is not forced down",
                    steps.join(", ")
                )
            };
            let (lo, hi) = g.region;
            format!(
                "`{}` in area `{}` (region [{}, {}, {}]..[{}, {}, {}], {}/{} cells filled at \
                 world-load): {fate}",
                g.anchor, g.area, lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], g.blocked, g.cells
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// **Where a player can walk from `seat`, under EVERY quest state that can hold
/// while that seat is the respawn point in force** (spec-0032).
///
/// Returns `(quest states examined, the intersected reachable set)`.
///
/// # Why an intersection, and why it lives here
///
/// spec-0032's placement rule says "under the quest state in force at that
/// moment". Nothing observable at runtime says WHICH point of a respawn point's
/// DAG span a death happened at — `#cp` names the seat, not the step — so the
/// table cannot key on quest state without inventing a runtime discriminator for
/// it. Intersecting instead makes the answer independent of that: a cell in this
/// set is reachable under every configuration the seat can be in force across, so
/// an anchor chosen from it is reachable whenever the player comes back.
///
/// **That is strictly stronger than the rule as written, not a simplification of
/// it** — the sentence a future reader needs, because it looks like a shortcut and
/// is the opposite. The rule permits an anchor reachable under the one quest state
/// that held at the moment of death; this permits only anchors reachable under all
/// of them.
///
/// It lives in this module, beside the model it reads, so `RegionState`,
/// [`World::region_state_at`] and [`World::with_region_state`] stay private: a second
/// passability model beside this one is exactly what spec-0031 refused when it
/// moved fill/clear out of the two verbs that held it privately.
///
/// A campaign with no runtime-written region has exactly one configuration and
/// pays one flood fill for it. Deterministic (ADR-0006): iteration is over a slice
/// in step order and over `BTreeSet` keys.
pub fn reachable_under_every_quest_state(
    plan: &Plan,
    world: &World,
    seat: [i32; 3],
    from_step: usize,
) -> (usize, BTreeSet<[i32; 3]>) {
    let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
    let mut seen: Vec<RegionState> = Vec::new();
    let mut acc: Option<BTreeSet<[i32; 3]>> = None;
    for arrival in from_step..=plan.critical_path.len() {
        let st = world.region_state_at(&plan.region_events, arrival, &ancestor);
        // Distinct configurations only. The number of them over a whole critical
        // path is small, and re-flooding an identical one would only cost time.
        // Compared WHOLE, never field by field. Two states that differ only in what
        // a runtime write flooded are two different worlds, and a hand-written
        // subset of the fields silently drops the newest one — which is exactly how
        // a third set gets added and the dedup keeps answering for two.
        if seen.contains(&st) {
            continue;
        }
        let w = if st.is_empty() {
            None
        } else {
            Some(world.with_region_state(&st))
        };
        let r = w.as_ref().unwrap_or(world).reachable_walkable(&[seat]);
        seen.push(st);
        acc = Some(match acc {
            None => r,
            Some(prev) => prev.intersection(&r).copied().collect(),
        });
    }
    // A seat whose span admits no configuration at all (its `from_step` is past the
    // last one) still gets the base world's answer rather than an empty set, which
    // would silently make every proof over it vacuous.
    (
        seen.len().max(1),
        acc.unwrap_or_else(|| world.reachable_walkable(&[seat])),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::*;
    use std::collections::BTreeSet;

    use crate::compiler::plan::RegionEvents;
    use crate::compiler::plan::{RegionEvent, RegionWrite};

    use crate::compiler::nav::*;

    // --- close-gate completability (DSL v0.6) --------------------------------

    /// A `close-gate` firing before a forced walked leg seals the gate region, so a
    /// critical path that must re-cross it fails DW0311; a later `open-gate` before
    /// the same leg reopens it and the route passes again.
    #[test]
    fn close_gate_seals_a_forced_leg_is_dw0311() {
        // A 1-wide corridor along x, y=65; the pass-through cell [2,65,0] is the sole
        // connection between the two ends. Base world (gate open) routes end to end.
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        assert!(
            route_visited(&world, &[a, b], &RegionEvents::default(), &linear).is_ok(),
            "the open corridor must route with no gate events"
        );
        // A close-gate seals the pass-through before the leg to `b` (fire_step 0 < 2).
        let close = RegionEvent::forced(([2, 65, 0], [2, 65, 0]), RegionWrite::Fill, 0);
        let err = route_visited(
            &world,
            &[a, b],
            &RegionEvents::from(vec![close.clone()]),
            &linear,
        )
        .unwrap_err();
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE); // DW0311
        assert!(
            err.message.contains("close-gate"),
            "the message must name the sealed gate: {}",
            err.message
        );
        // Reopening the gate before the leg (open-gate at a later fire_step) restores it.
        let open = RegionEvent::forced(([2, 65, 0], [2, 65, 0]), RegionWrite::Unseal, 1);
        assert!(
            route_visited(
                &world,
                &[a, b],
                &RegionEvents::from(vec![close.clone(), open.clone()]),
                &linear
            )
            .is_ok(),
            "a gate reopened by open-gate before the leg must route again"
        );
    }

    // --- the region write, generalised (DSL v0.10, spec-0031) ---------------

    /// A `fill-region` seals a forced leg exactly as a `close-gate` does, and a
    /// later `clear-region` over the same box reopens it.
    ///
    /// Same world, same predicate, same proof as the gate pair above — which is the
    /// claim: the completability rule belongs to the region, and a verb that names
    /// no gate inherits it rather than re-deriving it.
    #[test]
    fn fill_region_seals_a_forced_leg_and_clear_region_reopens_it() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let fill = RegionEvent::forced(([2, 65, 0], [2, 65, 0]), RegionWrite::Fill, 0);
        let err = route_visited(
            &world,
            &[a, b],
            &RegionEvents::from(vec![fill.clone()]),
            &linear,
        )
        .unwrap_err();
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE); // DW0311
        let clear = RegionEvent::forced(([2, 65, 0], [2, 65, 0]), RegionWrite::Clear, 1);
        assert!(
            route_visited(
                &world,
                &[a, b],
                &RegionEvents::from(vec![fill.clone(), clear.clone()]),
                &linear
            )
            .is_ok(),
            "a region cleared before the leg must route again"
        );
    }

    /// A floor at `y=64` with a three-cell gap at `x ∈ {1,2,3}` — the two ends are
    /// separated by open void, so nothing routes end to end until something lays
    /// floor in the gap.
    fn chasm() -> World {
        let mut solid = BTreeSet::new();
        for x in 0..5i32 {
            if !(1..=3).contains(&x) {
                solid.insert([x, 64, 0]); // floor, minus the gap
            }
            solid.insert([x, 67, 0]); // ceiling
        }
        World::from_solid_cells(solid)
    }

    /// A `fill-region` that LAYS floor — a repaired stair, a lowered bridge, a
    /// placed plank — carries the critical path across a gap, and the exported
    /// waypoints are judged in the same world the route was proven in.
    ///
    /// The two halves used to disagree about this world, and only one of them was
    /// wrong. The completability proof ([`route_visited`]) has read the leg's
    /// runtime region state since spec-0031; the waypoint self-check
    /// ([`verify_exported_routes`]) re-judged the very same cells against the BARE
    /// assembled world, where the plank does not exist. So a leg over runtime-laid
    /// floor routed and was then refused `DW0314` for having "no floor" — and a
    /// campaign whose critical path crosses a bridge it lowers could not ship.
    #[test]
    fn a_critical_path_over_runtime_laid_floor_routes_and_exports() {
        let world = chasm();
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        assert!(
            route_visited(&world, &[a, b], &RegionEvents::default(), &linear).is_err(),
            "the chasm must not route before anything lays floor in it"
        );
        // FORCED, and it has to be: this test asserts the leg routes and exports,
        // and only a fill the party cannot skip carries a forced leg's footing. The
        // unforced spelling of this very plank is the opposite verdict — see
        // `the_export_self_check_reads_a_legs_world_with_the_unforced_reading`.
        let plank = RegionEvent::forced(([1, 64, 0], [3, 64, 0]), RegionWrite::Fill, 0);
        assert!(
            route_visited(
                &world,
                &[a, b],
                &RegionEvents::from(vec![plank.clone()]),
                &linear
            )
            .is_ok(),
            "the proof must credit floor the campaign lays from a beat the party cannot skip, \
             before the leg is walked"
        );
        let legs: Vec<LegRoute> = route_walked_legs(
            &world,
            &[a, b],
            &RegionEvents::from(vec![plank.clone()]),
            &linear,
        )
        .into_iter()
        .map(|(leg, _)| leg)
        .collect();
        assert_eq!(legs.len(), 1, "the walked leg must be exported");
        assert!(
            legs[0].cells.contains(&[2, 65, 0]),
            "the exported route must cross the laid floor: {:?}",
            legs[0].cells
        );
        // The half that was wrong. Judged against the bare world these cells have
        // no floor; judged against the world the leg was proven over, they do.
        assert!(
            !world.is_standable([2, 65, 0]),
            "the bare assembled world really does lack the floor — otherwise this \
             test proves nothing"
        );
        verify_exported_routes(&world, &legs)
            .expect("a waypoint on floor the campaign lays must pass the export self-check");
    }

    /// The direction the self-check exists for is untouched: a cell a **later pass**
    /// mutated is still `DW0314`, because a terrain edit is not a runtime region
    /// write and no leg state restores it.
    ///
    /// Same leg, same laid plank, same exported route — but the final world has had
    /// one of the route's cells walled since the route was proven. The leg's own
    /// region state cannot explain that cell away, so the refusal stands.
    #[test]
    fn a_later_pass_that_walls_a_proven_cell_is_still_dw0314() {
        let world = chasm();
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        // FORCED for the same reason: the leg has to route at all before a later
        // pass can be shown to break it.
        let plank = RegionEvent::forced(([1, 64, 0], [3, 64, 0]), RegionWrite::Fill, 0);
        let legs: Vec<LegRoute> = route_walked_legs(
            &world,
            &[a, b],
            &RegionEvents::from(vec![plank.clone()]),
            &linear,
        )
        .into_iter()
        .map(|(leg, _)| leg)
        .collect();
        // A later pass drops a block into a cell the proven route walks through.
        let mutated = world.with_sealed(&[[2, 65, 0]].into_iter().collect());
        let err = verify_exported_routes(&mutated, &legs)
            .expect_err("a walled waypoint must still be refused");
        assert_eq!(err.code, DW_WAYPOINT_NOT_STANDABLE); // DW0314
        assert!(
            err.message.contains("[2, 65, 0]"),
            "the message must name the offending cell: {}",
            err.message
        );
    }

    /// **The junction of the two halves, at the one point where it is directly
    /// observable.** The export self-check judges a leg in the world the leg was
    /// proven over ([`LegRoute::proven_world`]), and that world is built by
    /// [`World::with_region_state`] — which also applies the unforced reading
    /// ([`World::with_unforced`]). So the self-check inherits forcedness for free,
    /// and neither half alone says so: one decided *which world* the check reads,
    /// the other decided *what an unforced fill does to a world*.
    ///
    /// Same world, same leg, same exported cells. The only thing that differs
    /// between the two verdicts below is whether the plank under `[2,65,0]` was
    /// laid by a beat the party cannot skip.
    ///
    /// **What would make this test vacuous**, stated so a later reader can check
    /// it rather than trust it:
    ///
    /// * If the chasm did not really lack the floor, the accept would pass for the
    ///   wrong reason — the bare world would already be standable and
    ///   `proven_world` would be doing nothing. Asserted below.
    /// * If the exported route did not really cross the laid cell, both verdicts
    ///   would be about a cell nobody stands on. Asserted below.
    /// * If `verify_exported_routes` returned `Err` for some unrelated cell, the
    ///   refusal would look right and mean nothing. The code is asserted, and the
    ///   message is required to name one of the cells standing on the plank.
    ///
    /// One thing this test deliberately does NOT claim: that a whole campaign can
    /// reach the refusing branch. It cannot — `route_visited` refuses an unforced
    /// footing as `DW0546` before any route is exported, so at campaign scale this
    /// reading is defence in depth rather than the live gate. That is why the
    /// unforced leg here is constructed rather than routed: `route_walked_legs`
    /// over an unforced plank correctly produces no leg at all, which is asserted
    /// too. The campaign-scale statement of the same junction is
    /// `crates/delvec/tests/laid_footing_root.rs`.
    #[test]
    fn the_export_self_check_reads_a_legs_world_with_the_unforced_reading() {
        let world = chasm();
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let box_ = ([1, 64, 0], [3, 64, 0]);

        // Not assumed: the bare assembled world really has no floor here, so
        // anything that stands at [2,65,0] is standing on something a runtime
        // write laid.
        assert!(
            !world.is_standable([2, 65, 0]),
            "the chasm must really be a chasm, or neither verdict below means anything"
        );

        // --- forced: the leg routes, and the export self-check accepts it -------
        let forced = RegionEvent::forced(box_, RegionWrite::Fill, 0);
        let legs: Vec<LegRoute> = route_walked_legs(
            &world,
            &[a, b],
            &RegionEvents::from(vec![forced.clone()]),
            &linear,
        )
        .into_iter()
        .map(|(leg, _)| leg)
        .collect();
        assert_eq!(legs.len(), 1, "the forced plank must carry a walked leg");
        assert!(
            legs[0].cells.contains(&[2, 65, 0]),
            "the exported route must really cross the laid cell: {:?}",
            legs[0].cells
        );
        // Reverting the leg-carries-its-world half reds HERE: judged against the
        // bare `world`, [2,65,0] has no floor and this becomes `DW0314`.
        verify_exported_routes(&world, &legs)
            .expect("footing the party cannot skip must pass the export self-check");

        // --- unforced: the same cells, in a world that may not hold the plank ---
        // `route_walked_legs` will not produce this leg — an unforced plank is
        // impassable and not floor, so nothing routes across it. That is the
        // campaign-scale verdict, and it is asserted rather than assumed.
        assert!(
            route_walked_legs(
                &world,
                &[a, b],
                &RegionEvents::from(vec![RegionEvent::unforced(
                    box_,
                    RegionWrite::Fill,
                    0,
                    "a trap nobody must spring"
                )]),
                &linear,
            )
            .is_empty(),
            "an unforced plank must not carry a walked leg at all"
        );
        // So the leg is carried over from the forced run with only its region
        // state re-marked: identical cells, identical world, one bit different.
        let mut unforced_state = RegionState::default();
        unforced_state
            .unforced
            .extend(crate::compiler::assembled::region_cells(box_.0, box_.1));
        let mut leg = legs[0].clone();
        leg.region_state = unforced_state;
        // Reverting the footing half reds HERE: with an unforced fill folded back
        // into `solid`, [2,65,0] is floored and the self-check accepts a waypoint
        // standing on a plank the party may never have laid.
        let err = verify_exported_routes(&world, std::slice::from_ref(&leg))
            .expect_err("a waypoint standing on unforced footing must not be exported");
        assert_eq!(err.code, DW_WAYPOINT_NOT_STANDABLE); // DW0314
        // The refusal must be ABOUT the plank, not about some other cell that
        // happens to be unstandable — that is the vacuity this assertion removes.
        // Which of the three cells standing on the box is reported is the loop's
        // order and not a claim, so any of them satisfies it.
        assert!(
            [[1, 65, 0], [2, 65, 0], [3, 65, 0]]
                .iter()
                .any(|c| err.message.contains(&format!("{c:?}"))),
            "the refusal must name a cell whose footing is the uncertain plank: {}",
            err.message
        );
    }

    /// A leg that writes no region judges the bare world, clones nothing, and is
    /// the answer it always was — the fast path every campaign without a runtime
    /// region takes.
    #[test]
    fn a_leg_with_no_runtime_write_judges_the_bare_world() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let legs: Vec<LegRoute> =
            route_walked_legs(&world, &[a, b], &RegionEvents::default(), &linear)
                .into_iter()
                .map(|(leg, _)| leg)
                .collect();
        assert_eq!(legs.len(), 1);
        assert!(
            legs[0].proven_world(&world).is_none(),
            "a leg with no runtime write must not clone a world"
        );
        verify_exported_routes(&world, &legs).expect("an ordinary leg still passes");
    }

    // --- what a write LEAVES: fluid is not floor ----------------------------

    /// A runtime fill of a **fluid** takes the floor away, and the model says so.
    ///
    /// The corridor's floor at `[2,64,0]` is the only footing between the two ends.
    /// A `Fill` there is a no-op (the cell was already solid) and the leg routes. A
    /// `Flood` there — the same box, the same fire step, the same verb, a different
    /// block — replaces the floor with water, and a body does not stand on water.
    ///
    /// This is the whole defect in four lines: with `Flood` folded into `Fill`, the
    /// second case routed too, and the compiler proved a party walking across a
    /// pond in mid-air.
    #[test]
    fn a_fluid_fill_takes_the_floor_away_and_a_solid_fill_does_not() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let floor_box = ([2, 64, 0], [2, 64, 0]);
        let solid_fill = RegionEvent::forced(floor_box, RegionWrite::Fill, 0);
        assert!(
            route_visited(
                &world,
                &[a, b],
                &RegionEvents::from(vec![solid_fill.clone()]),
                &linear
            )
            .is_ok(),
            "filling a floor cell with a block leaves it floor"
        );
        let fluid_fill = RegionEvent::forced(floor_box, RegionWrite::Flood, 0);
        let err = route_visited(
            &world,
            &[a, b],
            &RegionEvents::from(vec![fluid_fill.clone()]),
            &linear,
        )
        .unwrap_err();
        assert_eq!(err.code, DW_FLUID_FILL_ON_CRITICAL_PATH); // DW0544
        assert!(
            err.message.contains("[2, 64, 0]..[2, 64, 0]"),
            "the message must name the box that took the footing: {}",
            err.message
        );
    }

    /// A flooded cell blocks passage as hard as a wall does, on top of not being
    /// floor — so a fluid fill laid **across** the corridor closes it exactly as a
    /// `close-gate` would.
    ///
    /// The code here is `DW0311`, not `DW0544`, and that is the counterfactual being
    /// honest rather than a gap. `DW0544` answers one question — *would this route
    /// exist if the box held a block?* — and for a box laid across the path the
    /// answer is no: the campaign built a wall, and calling the fluid the culprit
    /// would send the author to change a block that changes nothing. What the fluid
    /// must still buy them is the right HINT: not "your prefab has a wedged
    /// doorway", which is the geometry-is-innocent misattribution this family exists
    /// to prevent.
    #[test]
    fn a_fluid_fill_across_the_corridor_is_a_wall_and_says_which() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let flood = RegionEvent::forced(([2, 65, 0], [2, 66, 0]), RegionWrite::Flood, 0);
        let err = route_visited(
            &world,
            &[a, b],
            &RegionEvents::from(vec![flood.clone()]),
            &linear,
        )
        .unwrap_err();
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE);
        assert!(
            err.message.contains("FLUID"),
            "an unroutable leg the campaign flooded must not be reported as a wedged \
             doorway: {}",
            err.message
        );
    }

    /// Where a fluid fill and a solid fill overlap, the **fluid** wins.
    ///
    /// Not a tie-break picked for convenience: a flooded cell is everything a walled
    /// cell is (impassable) and one thing more (not floor), so taking it is the
    /// conservative answer in the same sense that "a fill beats a clear" is. It also
    /// makes the result independent of declaration order, which ADR-0006 requires.
    #[test]
    fn where_a_fluid_fill_overlaps_a_solid_fill_the_fluid_wins() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let over_floor = RegionEvent::forced(([2, 64, 0], [2, 64, 0]), RegionWrite::Flood, 0);
        // A solid fill over a box that covers the same cell, declared either side of
        // the flood. Both orders must refuse.
        let wider_solid = RegionEvent::forced(([1, 64, 0], [3, 64, 0]), RegionWrite::Fill, 0);
        for events in [
            vec![over_floor.clone(), wider_solid.clone()],
            vec![wider_solid, over_floor],
        ] {
            let err = route_visited(
                &world,
                &[a, b],
                &RegionEvents::from(events.to_vec()),
                &linear,
            )
            .unwrap_err();
            assert_eq!(
                err.code, DW_FLUID_FILL_ON_CRITICAL_PATH,
                "a solid fill over the same cells must not dry the fluid out"
            );
        }
    }

    /// A runtime **clear** declared over a box a different write floods does not dry
    /// it: within one quest state the order is clear → fill → flood, so the wettest
    /// answer is the one that survives.
    ///
    /// This is [`World::with_cleared`]'s stated rule reaching a case it could not
    /// reach before — a `fill … air` against a wet cell lets the water back in
    /// rather than removing it, and the water may now be water a runtime write put
    /// there rather than only water a prefab did. What the ordering guards is a
    /// reorder of [`World::with_region_state`]: run the clear last and this campaign
    /// silently proves a dry floor again.
    #[test]
    fn a_clear_over_a_flooded_box_does_not_dry_it() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let flood = RegionEvent::forced(([2, 64, 0], [2, 64, 0]), RegionWrite::Flood, 0);
        // A different box (so it is a different region, with its own latest write)
        // covering the flooded floor cell and the air above it.
        let clear = RegionEvent::forced(([2, 64, 0], [2, 65, 0]), RegionWrite::Clear, 1);
        let err = route_visited(
            &world,
            &[a, b],
            &RegionEvents::from(vec![flood.clone(), clear.clone()]),
            &linear,
        )
        .unwrap_err();
        assert_eq!(err.code, DW_FLUID_FILL_ON_CRITICAL_PATH);
    }

    /// A campaign that writes no fluid pays nothing: the counterfactual is never
    /// built, and every existing verdict is the verdict it always was.
    #[test]
    fn a_world_with_no_runtime_flood_reports_none() {
        let world = floored(5, 1, 65, &[]);
        assert!(!world.has_runtime_flood());
        let sealed = world.with_sealed(&[[2, 65, 0]].into_iter().collect());
        assert!(
            !sealed.has_runtime_flood(),
            "a seal is not a flood, however many cells it forces"
        );
    }

    /// The half no gate could ever exercise: a `clear-region` credits a route
    /// through geometry the **prefab** put there, not merely through a wall an
    /// earlier effect built. The assembled model holds every gate cell open
    /// unconditionally, so `open-gate` never had to prove this and never did.
    #[test]
    fn clear_region_opens_prefab_geometry() {
        // A wall cell across the corridor: no route at all in the base world.
        let world = floored(5, 1, 65, &[[2, 65, 0], [2, 66, 0]]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        assert!(
            route_visited(&world, &[a, b], &RegionEvents::default(), &linear).is_err(),
            "the walled corridor must not route before the clear"
        );
        let clear = RegionEvent::forced(([2, 65, 0], [2, 66, 0]), RegionWrite::Clear, 0);
        assert!(
            route_visited(
                &world,
                &[a, b],
                &RegionEvents::from(vec![clear.clone()]),
                &linear
            )
            .is_ok(),
            "the cleared wall must be passable from the DAG point the clear fires at"
        );
    }

    /// An `open-gate` is **not** an unfiltered clear: it removes only the gate's own
    /// block. So it cannot delete geometry another proof has forced solid — a
    /// `collapse`'s debris resting in the doorway stays exactly where it fell.
    ///
    /// The guard is [`World::pinned`], and it holds for an authored `clear-region`
    /// too: clearing a region says "the blocks the campaign put here are gone", not
    /// "the hazard another proof is reasoning about never happened".
    #[test]
    fn a_runtime_clear_does_not_undo_another_proofs_premise() {
        let world = floored(5, 1, 65, &[]);
        let debris: BTreeSet<[i32; 3]> = [[2, 65, 0], [2, 66, 0]].into_iter().collect();
        let buried = world.with_sealed(&debris);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        assert!(
            route_visited(&buried, &[a, b], &RegionEvents::default(), &linear).is_err(),
            "the debris blocks the corridor"
        );
        for write in [RegionWrite::Unseal, RegionWrite::Clear] {
            let ev = RegionEvent::forced(([2, 65, 0], [2, 66, 0]), write, 0);
            assert!(
                route_visited(
                    &buried,
                    &[a, b],
                    &RegionEvents::from(vec![ev.clone()]),
                    &linear
                )
                .is_err(),
                "{write:?} must not delete another proof's forced-solid cells"
            );
        }
    }

    /// The seal is **DAG-causal**, not linear: a `close-gate` fired on a parallel
    /// quest branch (an ancestor of neither end of the leg) must NOT seal it, even
    /// though its `fire_step` is numerically earlier — the fix for the lineariser
    /// interleaving a sibling branch ahead of a sealed leg (island `take-the-cheese`
    /// vs `hide`). A genuinely-forced causal re-crossing is still sealed.
    #[test]
    fn close_gate_seal_is_dag_causal_not_linear() {
        let world = floored(5, 1, 65, &[]);
        let close = RegionEvent::forced(([2, 65, 0], [2, 65, 0]), RegionWrite::Fill, 8);
        let a = at_step([0, 65, 0], 9);
        let b = at_step([4, 65, 0], 10);
        // Parallel: the close (step 8), the prior position (step 9) and the arrival
        // (step 10) are three sibling branches — nothing at either end inherits it.
        let parallel = |g: usize, s: usize| !((g == 8 || g == 9) && (s == 9 || s == 10)) && g < s;
        assert!(
            route_visited(
                &world,
                &[a, b],
                &RegionEvents::from(vec![close.clone()]),
                &parallel
            )
            .is_ok(),
            "a close on a parallel branch must not seal a non-causal leg"
        );
        // Causal: step 8 (close) and step 9 are ancestors of step 10 (a forced
        // re-crossing with no reopen) → sealed → DW0311 (proof preserved).
        let err = route_visited(
            &world,
            &[a, b],
            &RegionEvents::from(vec![close.clone()]),
            &linear,
        )
        .expect_err("a forced causal re-crossing of a sealed gate must fail");
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE);
    }

    /// **A leg the ancestry does not connect is still judged.** The start (step
    /// 9) is not an ancestor of the arrival (step 10), but the close (step 8)
    /// precedes the start: the party standing at the start has shut the gate it
    /// is about to cross. Read over the open world, this leg built clean.
    #[test]
    fn an_unconnected_leg_is_sealed_by_what_its_start_has_fired() {
        let world = floored(5, 1, 65, &[]);
        let close = RegionEvent::forced(([2, 65, 0], [2, 65, 0]), RegionWrite::Fill, 8);
        let a = at_step([0, 65, 0], 9);
        let b = at_step([4, 65, 0], 10);
        // The close is the start's ancestor; neither it nor the start is the
        // arrival's.
        let unconnected = |g: usize, s: usize| !((g == 8 || g == 9) && s == 10) && g < s;
        let err = route_visited(
            &world,
            &[a, b],
            &RegionEvents::from(vec![close.clone()]),
            &unconnected,
        )
        .expect_err("the start's own close shuts the leg");
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE);
        // The start's own firing counts too: the close fires AT step 9.
        let at_start = RegionEvent::forced(([2, 65, 0], [2, 65, 0]), RegionWrite::Fill, 9);
        let err = route_visited(
            &world,
            &[a, b],
            &RegionEvents::from(vec![at_start.clone()]),
            &unconnected,
        )
        .expect_err("the close the start step fires shuts the leg");
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE);
        // And the exported route agrees with the proof: no leg is routed.
        assert!(
            route_walked_legs(
                &world,
                &[a, b],
                &RegionEvents::from(vec![close.clone()]),
                &unconnected
            )
            .is_empty(),
            "the harness is never handed a route the proof refused"
        );
    }

    // --- unforced footing (DW0546) -------------------------------------------

    /// A corridor whose floor is missing at `x = 2`: the two ends are separated by a
    /// one-cell void gap, so nothing routes end to end until something floors it.
    fn gapped_floor() -> World {
        let mut solid = BTreeSet::new();
        for x in 0..5i32 {
            if x != 2 {
                solid.insert([x, 64, 0]); // floor, minus the gap
            }
            solid.insert([x, 67, 0]); // ceiling
        }
        World::from_solid_cells(solid)
    }

    /// **The rule, at the layer it lives on.** The identical fill over the identical
    /// box carries the forced leg when the party cannot avoid causing it, and does
    /// not when they can. Only the root differs.
    ///
    /// Red before forcedness reached the geometry: both cases routed, because a fill
    /// was a fill and the model had no way to say who had to fire it.
    #[test]
    fn only_a_forced_fill_lays_footing_the_critical_path_may_use() {
        let world = gapped_floor();
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let gap = ([2, 64, 0], [2, 64, 0]);

        let forced = RegionEvent::forced(gap, RegionWrite::Fill, 0);
        assert!(
            route_visited(
                &world,
                &[a, b],
                &RegionEvents::from(vec![forced.clone()]),
                &linear
            )
            .is_ok(),
            "a plank the party cannot avoid laying is floor they certainly have"
        );

        let unforced = RegionEvent::unforced(gap, RegionWrite::Fill, 0, "the payload of trap `t`");
        let err = route_visited(
            &world,
            &[a, b],
            &RegionEvents::from(vec![unforced.clone()]),
            &linear,
        )
        .expect_err("a plank laid by a skippable beat may not carry the forced path");
        assert_eq!(err.code, DW_UNFORCED_FOOTING); // DW0546
        assert!(
            err.message.contains("[2, 64, 0]..[2, 64, 0]"),
            "the message must name the box: {}",
            err.message
        );
        assert!(
            err.message.contains("the payload of trap `t`"),
            "the message must name the beat: {}",
            err.message
        );
    }

    /// The blocking half of an unforced write is credited in FULL — only the footing
    /// half is withheld. A fill laid across the corridor closes the leg whoever fires
    /// it, and the author is told which write walled them rather than sent to hunt a
    /// wedged doorway.
    ///
    /// This is the half that keeps the fix from being a quiet weakening: an unforced
    /// write is strictly *more* restrictive than a forced one, never less.
    #[test]
    fn an_unforced_fill_still_seals_and_says_which() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let wall = RegionEvent::unforced(
            ([2, 65, 0], [2, 66, 0]),
            RegionWrite::Fill,
            0,
            "the payload of trap `t`",
        );
        let err = route_visited(
            &world,
            &[a, b],
            &RegionEvents::from(vec![wall.clone()]),
            &linear,
        )
        .expect_err("an unforced fill across the corridor is still a wall");
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE); // DW0311
        assert!(
            err.message.contains("close-gate") && err.message.contains("NOT forced"),
            "the message must name the unforced write, never blame the prefab: {}",
            err.message
        );
    }

    /// **The case that keeps this from refusing correct campaigns.** A fill over a
    /// cell the world already holds solid changes nothing about footing: the box is
    /// floor whether or not the beat fires, both futures agree, and there is no
    /// uncertainty to model. Re-surfacing an existing floor is decoration, and the
    /// rule binds to laying NEW floor.
    #[test]
    fn an_unforced_fill_over_existing_floor_is_not_a_finding() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let repave = RegionEvent::unforced(
            ([2, 64, 0], [2, 64, 0]),
            RegionWrite::Fill,
            0,
            "the payload of trap `t`",
        );
        assert!(
            route_visited(
                &world,
                &[a, b],
                &RegionEvents::from(vec![repave.clone()]),
                &linear
            )
            .is_ok(),
            "a fill over a cell that was already floor takes nothing away"
        );
    }

    /// A **forced** write landing later on the same box restores ordinary footing,
    /// with no special case: latest-write-wins already says which firing the party
    /// will find, and the winner carries its own forcedness.
    #[test]
    fn a_later_forced_fill_wins_over_an_earlier_unforced_one() {
        let world = gapped_floor();
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 3);
        let gap = ([2, 64, 0], [2, 64, 0]);
        let events = [
            RegionEvent::unforced(gap, RegionWrite::Fill, 0, "the payload of trap `t`"),
            RegionEvent::forced(gap, RegionWrite::Fill, 2),
        ];
        assert!(
            route_visited(
                &world,
                &[a, b],
                &RegionEvents::from(events.to_vec()),
                &linear
            )
            .is_ok(),
            "a beat the party must complete re-lays the plank for certain"
        );
    }

    /// An unforced **flood** needs no split and gets none: impassable and never floor
    /// is already the pointwise-worst of "the water is there" and "it is not", so it
    /// is judged exactly as a forced flood is — `DW0544`, not `DW0546`.
    #[test]
    fn an_unforced_flood_is_judged_as_a_flood() {
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let flood = RegionEvent::unforced(
            ([2, 64, 0], [2, 64, 0]),
            RegionWrite::Flood,
            0,
            "the payload of trap `t`",
        );
        let err = route_visited(
            &world,
            &[a, b],
            &RegionEvents::from(vec![flood.clone()]),
            &linear,
        )
        .expect_err("a fluid fill takes the floor away whoever fires it");
        assert_eq!(err.code, DW_FLUID_FILL_ON_CRITICAL_PATH); // DW0544
    }
}
