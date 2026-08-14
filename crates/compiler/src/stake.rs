//! The recovery stake's **compile-time placement table** (DSL v0.10, spec-0032),
//! the proofs it owes, and the binding ledger that says what they looked at.
//!
//! # The rule, and why it is a table
//!
//! Owner ruling, 2026-08-08:
//!
//! > The stake anchor is the point, on the walkable path from the respawn point in
//! > force at the moment of death to the death point under the quest state in force
//! > at that moment, that minimises distance to the death point.
//!
//! Read literally that is a search, and a search at runtime is exactly what
//! ADR-0006 forbids. Read carefully it is a *function of three compile-time
//! quantities*, and every one of them already has an owner in this compiler:
//!
//! * **walkable** is [`crate::nav::World`], the same navigation model the
//!   completability proof runs on — including a lethal volume's cells, which are
//!   impassable there, so "the near lip of the hazard" falls out rather than being
//!   a second rule;
//! * **under the quest state** is the DAG-indexed sealing `close-gate` established
//!   ([`crate::nav::seal_configurations`]);
//! * **the respawn point in force** is engine state the runtime already tracks in
//!   `#cp dw.sys`.
//!
//! So this module evaluates the rule for every (death region × respawn seat) pair
//! at build time and emission writes the answers out as a fixed `execute if …`
//! chain. There is no runtime search and no nondeterminism.
//!
//! # The rule degenerates, and that is why there is only one rule
//!
//! Written as *"the reachable cell that minimises distance to the death point"*,
//! the ordinary case answers itself: a player who dies on walkable ground they
//! could walk back to is at distance zero from themselves, so the anchor is the
//! death point and no second rule is needed. Only the deaths whose position
//! **cannot** host a stake need a table row, and there are exactly two kinds:
//!
//! 1. a death inside a **lethal volume** — the case that commissioned the rule;
//! 2. a death on a block **runtime can remove** — a lift car, a `close-gate`
//!    region, a `fill-region` or `clear-region` volume, a `collapse`'s floor.
//!    spec-0031's ruling: *a recovery stake may never be placed on a block that
//!    runtime can remove*, because the next ride would delete it. That is not a
//!    separate mechanism; it is the same projection, applied to a second family
//!    of place a stake may not stand.
//!
//! # The three ways a stake can be pulled out from under itself, and which are here
//!
//! Two are, one is not, and the third is named rather than left silent.
//!
//! * **Runtime-mutable ground** (`close-gate`, `set-block`, `collapse`, a
//!   shortcut's or a timed gate's seal) — in scope, and the case the ruling was
//!   written for. `DW0526`.
//! * **`fill-region` / `clear-region`** (spec-0031) — in scope, and *the same
//!   defect*: a `clear-region` deletes the block a marker stands on exactly as a
//!   departing lift car does. They enter through
//!   [`runtime_mutable_regions`] by way of `QuestEffect::region_write`, the DSL's
//!   own answer to "which verbs rewrite a box", so a later verb of that family is
//!   covered by existing rather than by being remembered.
//! * **A `teleport`'s `from` box** (spec-0031) — **NOT in scope, and this is a
//!   deliberate ruling rather than an omission.** A teleport moves *entities*,
//!   not blocks, so the ground under a marker is untouched; what moves is the
//!   marker itself, away from the position the collecting player's ledger
//!   recorded. That is a different defect with a different fix, and it is not one
//!   a box check on `DW0526`'s axis could state: `DW0526` is about **footing**,
//!   and a marker's position is chosen at RUNTIME, so no compile-time geometry
//!   test can know where it will be. Recorded as a follow-up finding rather than
//!   bolted onto a rule it does not belong to.
//!
//!   The reason it cannot simply inherit the teleport's own `DW0542` is worth
//!   stating, because it is the shape spec-0031 warned about when it refused to
//!   inherit `lethal_volumes[]`'s exemption list into a verb that *moves* rather
//!   than *deletes*: `DW0542` tests the affordance authority, which carries
//!   compile-time cells, and a stake has none to offer it. Inheriting the list
//!   would have produced a green that examined nothing.
//!
//! Both are boxes, so both are a selector the corpse can be tested against at
//! runtime (`@s[x=…,dx=…]`), which is what makes the lookup a comparison rather
//! than a search.
//!
//! # What is deliberately conservative, and stated rather than hidden
//!
//! spec-0032 already records one honest imprecision — *reachable under the quest
//! state* stands in for *explored*, which the engine does not track. This module
//! adds a second, in the same direction:
//!
//! **The quest-state axis is collapsed by intersection, not by choosing one
//! state.** A respawn seat is in force across a whole span of the quest DAG, and
//! nothing observable at runtime says which point of that span a death happened
//! at. So the reachable set used for a seat is the **intersection** of the
//! reachable sets over every sealing configuration that can hold while that seat
//! is in force. The anchor is then reachable under *all* of them, which is
//! strictly stronger than the rule as written and needs no runtime discriminator
//! for quest state at all. A campaign with no `close-gate` has exactly one
//! configuration, so it pays nothing for this.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::Campaign;

use crate::nav::{NavError, World};
use crate::plan::Plan;
use delvewright_dsl::DwCode;

/// `DW0525`: a death region has **no walkable route back** — from some respawn
/// seat, under some quest state, there is no reachable cell at all that a stake
/// left for a death in that region could stand on.
///
/// This is spec-0032's acceptance criterion 8, and it is the failure a souls-shaped
/// delve produces by accident: a one-way drop. You fall, you die, the engine leaves
/// your purse at the bottom, and the way back does not exist. The message names the
/// death region and the quest state, because those are the two things the author has
/// to change.
pub const DW_STAKE_NO_ROUTE_BACK: DwCode = DwCode::every_version("DW0525");

/// `DW0526`: every cell a stake could be projected onto for a death region sits on
/// a block **runtime removes** — so the marker would be destroyed by the next ride,
/// the next seal, or the next collapse.
///
/// Distinguished from [`DW_STAKE_NO_ROUTE_BACK`] because the prescription is
/// opposite: there *is* a route back, and the ground it ends on is the problem.
pub const DW_STAKE_UNSAFE_ANCHOR: DwCode = DwCode::every_version("DW0526");

/// One respawn seat the table is keyed on: the value `#cp dw.sys` holds while it is
/// in force, a human label, the standable cell, and the earliest critical-path step
/// at which it can be in force.
#[derive(Clone, Debug)]
pub struct Seat {
    /// `-1` for the campaign's entry spawn (the respawn point before any checkpoint
    /// has been set), otherwise the checkpoint's `#cp` index.
    pub cp: i32,
    /// What a diagnostic calls it.
    pub label: String,
    /// The standable cell the player comes back to.
    pub cell: [i32; 3],
    /// The earliest critical-path step at which this seat can be in force. Used to
    /// discard sealing configurations that cannot hold while it is.
    pub from_step: usize,
}

/// One death region: a box whose deaths cannot host a stake, and why.
#[derive(Clone, Debug)]
pub struct DeathRegion {
    /// What a diagnostic calls it.
    pub label: String,
    /// Inclusive world-space corners.
    pub region: ([i32; 3], [i32; 3]),
    /// Whether this region is a lethal volume (as opposed to runtime-mutable
    /// ground). Carried so a diagnostic can give the right prescription.
    pub lethal: bool,
}

/// One row of the table: *a death in this region, with this seat in force, leaves
/// its stake at this anchor.*
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Index into [`StakeTable::seats`].
    pub seat: usize,
    /// Index into [`StakeTable::regions`].
    pub region: usize,
    /// Index into [`StakeTable::anchors`].
    pub anchor: usize,
}

/// The whole compile-time answer: the axes, the distinct anchors, and the rows.
#[derive(Clone, Debug, Default)]
pub struct StakeTable {
    /// Respawn seats, `#cp`-ordered (entry first).
    pub seats: Vec<Seat>,
    /// Death regions, lethal volumes first (declaration order), then
    /// runtime-mutable regions (source order).
    pub regions: Vec<DeathRegion>,
    /// The distinct anchors the rows point at, in first-use order.
    pub anchors: Vec<[i32; 3]>,
    /// One row per (seat, region) pair, in `(seat, region)` order.
    pub rows: Vec<Row>,
    /// The binding ledger.
    pub gate: StakeGate,
}

impl StakeTable {
    /// The anchor for a (seat, region) pair, if the table has a row for it.
    pub fn anchor_of(&self, seat: usize, region: usize) -> Option<[i32; 3]> {
        self.rows
            .iter()
            .find(|r| r.seat == seat && r.region == region)
            .map(|r| self.anchors[r.anchor])
    }
}

/// What the stake proofs actually examined.
///
/// CLAUDE.md: *a green gate that binds to nothing is vacuous, not a pass.* Every
/// number here is a count of things looked at, not of findings — a zero in
/// `regions` or `seats` means the table proved nothing, and [`Self::unbound`] says
/// so out loud rather than leaving a reader to infer it from a silent pass.
#[derive(Clone, Debug, Default)]
pub struct StakeGate {
    /// Stakes the campaign declared.
    pub declared: usize,
    /// Respawn seats the table is keyed on.
    pub seats: usize,
    /// Death regions the table is keyed on.
    pub regions: usize,
    /// Of those, how many are lethal volumes (the rest are runtime-mutable ground).
    pub lethal_regions: usize,
    /// Distinct sealing configurations — the quest-state axis — enumerated.
    pub configurations: usize,
    /// Rows proved: one per (seat, region) pair.
    pub rows: usize,
    /// Distinct anchors the rows resolved to.
    pub anchors: usize,
    /// Cells excluded from every candidate set because runtime removes their
    /// support.
    pub mutable_cells: usize,
    /// Cells that are reachable from the entry but **not** from some seat — the
    /// one-way-drop set, which must be empty or `DW0525` fires.
    pub stranded_cells: usize,
}

impl StakeGate {
    /// Whether these proofs matched nothing at all. A campaign that declares a
    /// stake and has neither a seat nor a region to key on has proved nothing
    /// about where its stakes land.
    pub fn unbound(&self) -> bool {
        self.declared == 0 || self.seats == 0
    }

    /// The ledger as the `validation/stake-gate.json` artifact.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "stakes_declared": self.declared,
            "respawn_seats": self.seats,
            "death_regions": { "total": self.regions, "lethal": self.lethal_regions },
            "quest_state_configurations": self.configurations,
            "rows_proved": self.rows,
            "distinct_anchors": self.anchors,
            "runtime_mutable_cells_excluded": self.mutable_cells,
            "stranded_cells": self.stranded_cells,
            "unbound": self.unbound(),
        })
    }
}

/// A labelled inclusive box: what a diagnostic calls a region, and where it is.
///
/// A `type` rather than a bare tuple because it travels between four functions and
/// clippy is right that `Vec<LabelledBox>` reads as noise.
pub type LabelledBox = (String, ([i32; 3], [i32; 3]));

/// Squared distance from a cell to the nearest point of an inclusive box, in
/// integer cell units. Deterministic by construction (no floats — ADR-0006).
fn box_dist2(c: [i32; 3], region: ([i32; 3], [i32; 3])) -> i64 {
    let (lo, hi) = region;
    (0..3)
        .map(|i| {
            let d = if c[i] < lo[i] {
                lo[i] - c[i]
            } else if c[i] > hi[i] {
                c[i] - hi[i]
            } else {
                0
            } as i64;
            d * d
        })
        .sum()
}

/// Whether a cell lies inside an inclusive box.
fn in_box(c: [i32; 3], region: ([i32; 3], [i32; 3])) -> bool {
    let (lo, hi) = region;
    (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i])
}

/// **Every cell whose block the runtime can remove or replace**, with the region
/// each came from.
///
/// This set did not exist before spec-0032, and its absence is why the ruling *"a
/// stake may never sit on a block runtime can remove"* had nowhere to live: the
/// compiler knew the geometry of every runtime block write individually and had
/// never collected them. Every source below is already resolved on the [`Plan`];
/// nothing new is computed, it is only gathered in one place.
///
/// Returned as `(label, box)` in a fixed order — gate anchors (the `open-gate` /
/// `close-gate` regions), then shortcut gates, then timed gates, then `set-block`
/// anchors — so the emitted table is deterministic.
pub fn runtime_mutable_regions(plan: &Plan) -> Vec<LabelledBox> {
    let mut out: Vec<LabelledBox> = Vec::new();
    let mut seen: BTreeSet<([i32; 3], [i32; 3])> = BTreeSet::new();
    let mut push = |label: String, region: ([i32; 3], [i32; 3]), out: &mut Vec<_>| {
        if seen.insert(region) {
            out.push((label, region));
        }
    };
    // Gate anchors. `open-gate` and `close-gate` both `fill` the anchor's whole
    // region, so every cell of it is one the runtime rewrites — and the anchors map
    // is the one place a gate's geometry is resolved.
    for ((area, name), resolved) in &plan.anchors {
        if let crate::plan::ResolvedAnchor::Gate { from, to, .. } = resolved {
            push(
                format!("gate anchor `{name}` in area `{area}`"),
                (*from, *to),
                &mut out,
            );
        }
    }
    for sc in &plan.shortcuts {
        push(
            format!("shortcut `{}`'s gate", sc.id),
            sc.gate_region,
            &mut out,
        );
    }
    for tg in &plan.timed_gates {
        push(
            format!("timed gate `{}`'s gate", tg.id),
            tg.gate_region,
            &mut out,
        );
    }
    // Effect-level block writes. `region_write()` is the DSL's own answer to
    // "which verbs rewrite a box" (spec-0031) — `fill-region` and `clear-region`
    // today, and whatever a later version adds — so reading it means this set
    // widens with the language instead of with somebody's memory. `set-block`
    // rewrites one cell at its anchor and `collapse` empties its declared volume;
    // neither is a region write, so both are named here.
    //
    // **Why this is not `plan.region_events`**, which is the completability
    // model's list of the same thing. That list deliberately DROPS a non-fill
    // write fired from an optional root (`collect_region_events`: an optional
    // firing may fill, never open), because a proof about routes must not lean on
    // a clear the party might never trigger. This set needs the opposite
    // conservatism: a `clear-region` in a trap payload the party may never spring
    // is still ground a stake must not stand on, because if they DO spring it the
    // marker is gone. Same geometry, opposite direction, so the two lists cannot
    // be one — and saying so here is cheaper than the next reader assuming they
    // should be.
    //
    // Read through `timeline::walk`, the one traversal that reaches every effect
    // at every root including nested `sequence` steps — a hand-rolled walk here
    // would be the #301/#302/#321 defect in a new place.
    for (eff, _) in crate::timeline::walk(plan) {
        if let Some((zone, block)) = eff.region_write()
            && let Some(r) = plan.zone_box(zone)
        {
            let verb = if block.is_some() {
                "`fill-region`"
            } else {
                "`clear-region`"
            };
            push(
                format!("the {verb} volume at anchor `{}`", zone.anchor),
                r,
                &mut out,
            );
        }
        match eff {
            delvewright_dsl::QuestEffect::SetBlock { anchor, .. } => {
                if let Some(cell) = plan.point_any(anchor.as_str()) {
                    push(
                        format!("`set-block` anchor `{anchor}`"),
                        (cell, cell),
                        &mut out,
                    );
                }
            }
            delvewright_dsl::QuestEffect::Collapse { region_anchor, .. } => {
                if let Some(r) = plan.zone_box(region_anchor) {
                    push(
                        format!("`collapse` volume at anchor `{}`", region_anchor.anchor),
                        r,
                        &mut out,
                    );
                }
            }
            _ => {}
        }
    }
    out
}

/// The cells a stake may not stand **on**: a cell whose supporting block (the cell
/// directly below the feet cell) is one runtime removes, or which is itself inside
/// such a region (the runtime could fill it solid and bury the marker).
fn unsafe_footing(regions: &[LabelledBox]) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    for (_, r) in regions {
        for cell in crate::assembled::region_cells(r.0, r.1) {
            out.insert(cell);
            // The feet cell standing ON this block.
            out.insert([cell[0], cell[1] + 1, cell[2]]);
        }
    }
    out
}

/// **The rule itself**, over already-computed sets: the cell reachable from a seat
/// that minimises distance to a death region, excluding the region itself and any
/// ground the runtime rewrites.
///
/// Split out from [`build`] so the rule can be exercised on stated inputs rather
/// than only through a whole campaign — the two failure modes it distinguishes are
/// geometric facts about three sets, and a test that has to build a world to reach
/// them is a test of the world.
///
/// Ties break lexicographically on the cell, so the answer is deterministic
/// (ADR-0006) and independent of the iteration order of the set handed in.
///
/// # Errors
///
/// [`DW_STAKE_NO_ROUTE_BACK`] when nothing outside the region is reachable at all;
/// [`DW_STAKE_UNSAFE_ANCHOR`] when something is, but every candidate stands on
/// ground the runtime removes.
pub fn choose_anchor(
    reachable: &BTreeSet<[i32; 3]>,
    unsafe_cells: &BTreeSet<[i32; 3]>,
    region: ([i32; 3], [i32; 3]),
) -> Result<[i32; 3], DwCode> {
    let outside: Vec<[i32; 3]> = reachable
        .iter()
        .filter(|c| !in_box(**c, region))
        .copied()
        .collect();
    if outside.is_empty() {
        return Err(DW_STAKE_NO_ROUTE_BACK);
    }
    outside
        .iter()
        .filter(|c| !unsafe_cells.contains(*c))
        .min_by_key(|c| (box_dist2(**c, region), **c))
        .copied()
        .ok_or(DW_STAKE_UNSAFE_ANCHOR)
}

/// The respawn seats the table is keyed on, in `#cp` order (entry first).
///
/// `entry` is the campaign's resolved entry-spawn cell — the respawn point in force
/// before any checkpoint has been set, and therefore a real row of the table rather
/// than an edge case. Threaded in by the caller exactly as
/// [`crate::lethal::check_respawn_seats`] takes it.
///
/// Public because [`crate::deathplan`] hands the same list to the bot tier: "the
/// respawn point in force" is a promise a lethal volume makes with or without a
/// purse to lose, so a campaign that declares a volume and no stake still owes the
/// bot the seats to check its respawn against. Deriving them there would be a
/// second answer to one question.
pub fn seats(plan: &Plan, world: &World, entry: Option<[i32; 3]>) -> Vec<Seat> {
    let mut out = Vec::new();
    if let Some(pos) = entry
        && let Some(cell) = world.snap(pos, crate::nav::SNAP_RADIUS)
    {
        out.push(Seat {
            cp: -1,
            label: "the campaign's entry spawn".to_string(),
            cell,
            from_step: 0,
        });
    }
    for cp in &plan.checkpoints {
        let Some(cell) = world.snap(cp.pos, crate::nav::SNAP_RADIUS) else {
            // A checkpoint with no standable seat is already `DW0316`; this table
            // is not the place to re-report it.
            continue;
        };
        out.push(Seat {
            cp: cp.index as i32,
            label: format!(
                "{} anchor `{}`",
                if cp.rest { "bonfire" } else { "checkpoint" },
                cp.anchor
            ),
            cell,
            from_step: cp.fire_step,
        });
    }
    out
}

/// Build the placement table and discharge its proofs.
///
/// Returns `Ok(None)` for a campaign that declares no stake — the whole feature is
/// then absent from emission, which is what keeps every existing campaign
/// byte-identical.
///
/// # Errors
///
/// [`DW_STAKE_NO_ROUTE_BACK`] when a death region — or an ordinary walkable cell —
/// has no route back from some respawn seat, and [`DW_STAKE_UNSAFE_ANCHOR`] when
/// the only routes back end on ground the runtime removes.
pub fn build(
    plan: &Plan,
    world: &World,
    entry: Option<[i32; 3]>,
) -> Result<Option<StakeTable>, NavError> {
    let declared = plan.campaign.quests.content.stakes.len();
    if declared == 0 {
        return Ok(None);
    }
    let seats = seats(plan, world, entry);

    // --- the reachable set per seat, intersected over its quest states ---------
    // One call into the navigation module, which owns the region model: a "quest
    // state" is a `RegionState` (which regions are filled, which are cleared), and
    // computing it here would be a second passability model beside the one
    // spec-0031 spent a whole PR consolidating.
    let mut reach: Vec<BTreeSet<[i32; 3]>> = Vec::new();
    let mut configurations = 0usize;
    for seat in &seats {
        let (n, r) =
            crate::nav::reachable_under_every_quest_state(plan, world, seat.cell, seat.from_step);
        configurations = configurations.max(n);
        reach.push(r);
    }

    // --- the death regions ----------------------------------------------------
    let mutable = runtime_mutable_regions(plan);
    let unsafe_cells = unsafe_footing(&mutable);
    let mut regions: Vec<DeathRegion> = plan
        .lethal_volumes
        .iter()
        .map(|v| DeathRegion {
            label: format!("lethal volume `{}`", v.id),
            region: v.region,
            lethal: true,
        })
        .collect();
    let lethal_regions = regions.len();
    regions.extend(mutable.iter().map(|(label, r)| DeathRegion {
        label: format!("the runtime-mutable ground of {label}"),
        region: *r,
        lethal: false,
    }));

    // --- the ordinary case: every cell a player can die on must lead back ------
    // The rule's degenerate branch places the stake AT the death point, so the
    // obligation it carries is that the death point leads home. A one-way drop is
    // exactly the campaign that fails here, and it is `AC8`.
    let from_entry = match seats.first() {
        Some(s) if s.cp == -1 => reach[0].clone(),
        _ => world.reachable_walkable(&seats.iter().map(|s| s.cell).collect::<Vec<_>>()),
    };
    let mut stranded_cells = 0usize;
    for (i, seat) in seats.iter().enumerate() {
        let stranded: Vec<[i32; 3]> = from_entry
            .iter()
            .filter(|c| !reach[i].contains(*c))
            .filter(|c| !regions.iter().any(|r| in_box(**c, r.region)))
            .copied()
            .collect();
        stranded_cells += stranded.len();
        if let Some(first) = stranded.first() {
            return Err(NavError {
                code: DW_STAKE_NO_ROUTE_BACK,
                message: format!(
                    "a player can reach and die at {first:?} (and {} other cell(s)), and from {} \
                     there is no walkable route back to it under every quest state that can hold \
                     while that respawn point is in force. The stake the death leaves is placed \
                     where the player fell, so this campaign can strand a purse permanently — \
                     which is what `stakes[]` exists to make impossible. Either give the drop a \
                     way back (a shortcut, a ladder), or declare the place a `lethal_volume` so \
                     the stake is projected to its near lip instead. Do NOT delete the stake to \
                     silence this.",
                    stranded.len().saturating_sub(1),
                    seat.label,
                ),
            });
        }
    }

    // --- the rows -------------------------------------------------------------
    let mut anchors: Vec<[i32; 3]> = Vec::new();
    let mut anchor_index: BTreeMap<[i32; 3], usize> = BTreeMap::new();
    let mut rows: Vec<Row> = Vec::new();
    for (si, seat) in seats.iter().enumerate() {
        for (ri, region) in regions.iter().enumerate() {
            // The rule, over the sets computed above.
            let picked = choose_anchor(&reach[si], &unsafe_cells, region.region);
            let cell = match picked {
                Ok(c) => c,
                Err(code) => {
                    let why = if code == DW_STAKE_NO_ROUTE_BACK {
                        "no cell at all is reachable from that respawn point under every quest state \
                     that can hold while it is in force"
                    } else {
                        "every cell that IS reachable stands on a block the runtime removes — a lift \
                     car, a sealed gate region, a collapsed floor — so a marker left there would \
                     be destroyed by the next ride"
                    };
                    return Err(NavError {
                        code,
                        message: format!(
                            "a death in {} with {} in force has nowhere to leave its recovery stake: \
                         {why}. Quest states examined: {} (every distinct region state that can \
                         hold from that seat's step {} onward). Move the region, open a route back \
                         to it, or change what the runtime rewrites near it.",
                            region.label, seat.label, configurations, seat.from_step,
                        ),
                    });
                }
            };
            let idx = *anchor_index.entry(cell).or_insert_with(|| {
                anchors.push(cell);
                anchors.len() - 1
            });
            rows.push(Row {
                seat: si,
                region: ri,
                anchor: idx,
            });
        }
    }

    let gate = StakeGate {
        declared,
        seats: seats.len(),
        regions: regions.len(),
        lethal_regions,
        configurations,
        rows: rows.len(),
        anchors: anchors.len(),
        mutable_cells: unsafe_cells.len(),
        stranded_cells,
    };
    Ok(Some(StakeTable {
        seats,
        regions,
        anchors,
        rows,
        gate,
    }))
}

/// The declared stakes, for a caller that wants them without reaching into the
/// campaign twice.
pub fn declared(c: &Campaign) -> &[delvewright_dsl::Stake] {
    &c.quests.content.stakes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_distance_is_zero_inside_and_grows_outside() {
        let r = ([0, 0, 0], [2, 2, 2]);
        assert_eq!(box_dist2([1, 1, 1], r), 0);
        assert_eq!(box_dist2([3, 1, 1], r), 1);
        assert_eq!(box_dist2([-2, 1, 1], r), 4);
        // Diagonal: 3² + 4² over two axes.
        assert_eq!(box_dist2([5, 1, 6], r), 9 + 16);
    }

    #[test]
    fn unsafe_footing_covers_the_block_and_the_cell_above_it() {
        let regions = vec![("lift car".to_string(), ([0, 10, 0], [0, 10, 0]))];
        let s = unsafe_footing(&regions);
        assert!(s.contains(&[0, 10, 0]), "the block itself");
        assert!(s.contains(&[0, 11, 0]), "the feet cell standing on it");
        assert!(!s.contains(&[0, 12, 0]));
    }

    /// The rule, on stated sets: the reachable cell nearest the region wins, ties
    /// break lexicographically, and the region's own cells are never candidates.
    #[test]
    fn the_anchor_is_the_nearest_reachable_cell_outside_the_region() {
        let region = ([0, 0, 0], [0, 0, 0]);
        let reachable: BTreeSet<[i32; 3]> = [[0, 0, 0], [5, 0, 0], [2, 0, 0], [0, 0, 2]]
            .into_iter()
            .collect();
        assert_eq!(
            choose_anchor(&reachable, &BTreeSet::new(), region).unwrap(),
            [0, 0, 2],
            "distance 2 either way, and [0,0,2] is lexicographically first — the tie \
             break is part of the contract (ADR-0006)"
        );
    }

    /// `DW0525`: nothing outside the death region is reachable at all — the
    /// one-way drop.
    #[test]
    fn dw0525_fires_when_nothing_outside_the_region_is_reachable() {
        let region = ([0, 0, 0], [4, 4, 4]);
        let reachable: BTreeSet<[i32; 3]> = [[1, 1, 1], [2, 2, 2]].into_iter().collect();
        assert_eq!(
            choose_anchor(&reachable, &BTreeSet::new(), region),
            Err(DW_STAKE_NO_ROUTE_BACK)
        );
    }

    /// `DW0526`: there IS a route back, and every cell it ends on stands on ground
    /// the runtime rewrites — a lift car, a sealed gate region, a collapsed floor.
    /// A marker left there would be destroyed by the next ride (spec-0031's ruling).
    #[test]
    fn dw0526_fires_when_every_route_back_ends_on_ground_the_runtime_rewrites() {
        let region = ([0, 0, 0], [0, 0, 0]);
        let reachable: BTreeSet<[i32; 3]> = [[0, 0, 0], [3, 0, 0], [4, 0, 0]].into_iter().collect();
        let unsafe_cells: BTreeSet<[i32; 3]> = [[3, 0, 0], [4, 0, 0]].into_iter().collect();
        assert_eq!(
            choose_anchor(&reachable, &unsafe_cells, region),
            Err(DW_STAKE_UNSAFE_ANCHOR),
            "the distinction matters: DW0525 says there is no way back, DW0526 says \
             the way back ends on ground that will not hold a marker"
        );
        // …and with the same sets minus the unsafe marking, the same call succeeds,
        // so the test above is not passing for some other reason.
        assert_eq!(
            choose_anchor(&reachable, &BTreeSet::new(), region).unwrap(),
            [3, 0, 0]
        );
    }

    /// An empty ledger reports itself unbound rather than passing silently.
    #[test]
    fn a_gate_with_no_stake_is_unbound() {
        let g = StakeGate::default();
        assert!(g.unbound());
        assert_eq!(g.to_json()["unbound"], serde_json::json!(true));
    }
}
