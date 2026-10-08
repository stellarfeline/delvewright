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
                    RegionWrite::Fill | RegionWrite::Flood => {
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
            // futures); `Clear` and `Unseal` never reach here unforced, because
            // `plan::collect_region_events` drops them — an unforced firing may make
            // a region impassable and may never make one passable.
            let into = match write {
                RegionWrite::Fill if !forced => {
                    st.unforced_regions.push((region, blame));
                    &mut st.unforced
                }
                RegionWrite::Fill => &mut st.solid,
                RegionWrite::Clear => &mut st.cleared,
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
