//! The map editor's edit-script replay (DSL v0.6, spec-0017).
//!
//! Replays the optional stage-7 `world-edits.json` deterministically over the
//! assembled world, **after** assembly and before every downstream consumer —
//! so relight, nav, wave seating, waypoint export, snapshots and the emitted
//! datapack all see the *edited* world. The edit script is the artifact of
//! record (ADR-0006): same DSL + same edits + same seed → byte-identical world;
//! the replay never mutates world files as truth.
//!
//! ## Invariants (spec-0017, machine-enforced after EVERY batch)
//!
//! After each batch the replay re-settles gravity and re-proves, with the batch
//! named in any failure:
//!
//! 1. **Gravity**: an edit that places a falling block that would despawn into
//!    the void is rejected (`DW0313`, reused).
//! 2. **Sealing + relight**: the spec-0010 relight pass re-runs over the edited
//!    world (`DW0210`/`DW0211`, reused).
//! 3. **Walkability**: the critical-path and checkpoint proofs re-run
//!    (`DW0311`/`DW0315`/`DW0316`, reused) with the relight fixtures included,
//!    exactly as the final build's nav phase does.
//! 4. **Boundary safety** (`DW0322`, [`crate::nav::verify_boundary_safety`]):
//!    no reachable walkable cell may border a void drop.
//!
//! ## Runtime materialization
//!
//! The replay also lowers every batch to vanilla `fill`/`setblock` lines
//! (x-run coalesced, deterministic order) emitted as the `world_edits`
//! function, called from `setup_finish` after the socket seals and before the
//! relight fixtures — the same order the compile-time model applies them in.
//!
//! ## Determinism (ADR-0006)
//!
//! Every seeded verb derives its stream from the campaign seed + its script
//! position (`solver::stream_seed(seed, "edits/<batch-id>/<edit-index>")`) and
//! samples position-addressed value noise (the island/cave generators' proven
//! primitive family, ported here) — no wall clock, no unseeded RNG, no
//! iteration-order dependence.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{EditFrame, MorphOp, PaletteRecipe, RegionShape, WorldEdit};

use crate::assembled::{self, Assembled};
use crate::plan::{Plan, ResolvedAnchor};
use crate::solver::stream_seed;

/// A stage-7 edit's frame or region fails to resolve against the solved layout:
/// a piece index out of range, a piece whose prefab differs from the frame's
/// declared one (layout drift), an anchor the area does not resolve, or a verb
/// whose target region resolves to zero cells (a silent no-op is always a
/// defect). Build-tier (exit 3).
pub const DW_EDIT_UNRESOLVED: &str = "DW0323";

/// A failed edit replay: a stable diagnostic code plus a message that names the
/// offending batch.
#[derive(Debug)]
pub struct EditError {
    /// Stable `DW####` code (may be a reused invariant code, e.g. `DW0311`).
    pub code: &'static str,
    /// Human-readable message (remediation-contract style), naming the batch.
    pub message: String,
}

/// One replayed batch's outcome, for snapshot rendering and reporting.
pub struct BatchOutcome {
    /// The batch id (`batch/<kebab>`).
    pub id: String,
    /// The union AABB of every cell the batch wrote, or `None` for a batch that
    /// wrote nothing (structurally impossible today — every verb writes or
    /// errors — but kept honest).
    pub bounds: Option<([i32; 3], [i32; 3])>,
}

/// A completed edit replay over the assembled world.
pub struct EditReplay {
    /// The edited, re-settled assembled world every downstream consumer uses.
    pub assembled: Assembled,
    /// The runtime materialization: coalesced `fill`/`setblock` lines for the
    /// `world_edits` function, in application order.
    pub commands: Vec<String>,
    /// Per-batch outcomes, in script order.
    pub batches: Vec<BatchOutcome>,
}

/// Whether the campaign carries a non-empty edit script. `false` keeps the
/// build on the exact pre-stage-7 code path (byte-identical output).
pub fn has_edits(campaign: &delvewright_dsl::Campaign) -> bool {
    campaign
        .world_edits
        .as_ref()
        .is_some_and(|e| !e.content.batches.is_empty())
}

/// Replay the campaign's edit script over the assembled world, enforcing the
/// per-batch invariants. Returns `None` when the campaign has no (non-empty)
/// edit script — callers then take the unmodified legacy path. See the module
/// docs for the per-batch invariants.
pub fn replay(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
) -> Result<Option<EditReplay>, EditError> {
    replay_with(plan, structures, true)
}

/// [`replay`] for **view** commands (`delvec snapshot` / `blocking-chart`): the
/// edits are applied and gravity re-settles, but the invariant re-proofs are
/// skipped — a view command must be able to SHOW a world state whose invariants
/// fail (that is exactly what the author needs to look at). Region-resolution
/// failures still error: an unresolvable edit has no world state to show.
pub fn replay_view(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
) -> Result<Option<EditReplay>, EditError> {
    replay_with(plan, structures, false)
}

fn replay_with(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
    enforce: bool,
) -> Result<Option<EditReplay>, EditError> {
    if !has_edits(plan.campaign) {
        return Ok(None);
    }
    let env = plan
        .campaign
        .world_edits
        .as_ref()
        .expect("has_edits checked");

    let mut assembled = assembled::assemble(plan, structures);
    let mut commands: Vec<String> = Vec::new();
    let mut batches: Vec<BatchOutcome> = Vec::new();

    for batch in &env.content.batches {
        let bid = batch.id.as_str();
        let area = plan
            .areas
            .iter()
            .find(|a| a.area_id == batch.area.as_str())
            .ok_or_else(|| EditError {
                code: DW_EDIT_UNRESOLVED,
                message: format!(
                    "world-edits batch `{bid}` targets area `{}` which the plan did not place — \
                     use a stage-1 area id",
                    batch.area
                ),
            })?;

        // Named regions of this batch (strictly backward references, validated).
        let mut regions: BTreeMap<String, BTreeSet<[i32; 3]>> = BTreeMap::new();
        // Every cell this batch wrote, with the full (blockstate-carrying) form
        // for runtime materialization. Application order within the batch is the
        // verbs' order; within a verb, deterministic cell order.
        let mut batch_writes: BTreeMap<[i32; 3], String> = BTreeMap::new();

        for (ei, edit) in batch.edits.iter().enumerate() {
            let seed = stream_seed(plan.seed, &format!("edits/{bid}/{ei}"));
            match edit {
                WorldEdit::Select { name, shape } => {
                    let cells = resolve_shape(plan, area, bid, &regions, &assembled, shape)
                        .map_err(|message| EditError {
                            code: DW_EDIT_UNRESOLVED,
                            message,
                        })?;
                    regions.insert(name.as_str().to_string(), cells);
                }
                WorldEdit::Fill { region, recipe } => {
                    let cells = used_region(bid, &regions, region.as_str())?;
                    for &cell in cells {
                        let block = pick(recipe, noise_at(seed, cell, recipe));
                        write_cell(&mut assembled, &mut batch_writes, cell, block);
                    }
                }
                WorldEdit::Replace {
                    region,
                    matching,
                    recipe,
                } => {
                    let cells = used_region(bid, &regions, region.as_str())?;
                    let matches: BTreeSet<&str> =
                        matching.iter().map(|m| strip_ns(base_id(m))).collect();
                    for &cell in cells {
                        let Some(current) = assembled.blocks.get(&cell) else {
                            continue; // air never matches a base id
                        };
                        if !matches.contains(strip_ns(current.as_str())) {
                            continue;
                        }
                        let block = pick(recipe, noise_at(seed, cell, recipe));
                        write_cell(&mut assembled, &mut batch_writes, cell, block);
                    }
                }
                WorldEdit::Carve { region } => {
                    let cells = used_region(bid, &regions, region.as_str())?;
                    for &cell in cells {
                        write_cell(&mut assembled, &mut batch_writes, cell, "minecraft:air");
                    }
                }
                WorldEdit::Morph { region, op } => {
                    let cells = used_region(bid, &regions, region.as_str())?;
                    morph(&mut assembled, &mut batch_writes, cells, op, seed);
                }
            }
        }

        // Re-settle gravity over the edited map (the runtime edits are
        // `setblock`/`fill` on a live world — a falling block placed unsupported
        // falls, exactly like placement). A despawn is always a defect (DW0313's
        // rule), attributed to this batch.
        let settled = assembled::resettle(&mut assembled.blocks);
        if enforce && let Some(lost) = settled.iter().find(|s| s.to.is_none()) {
            return Err(EditError {
                code: assembled::DW_GRAVITY_DESPAWN,
                message: format!(
                    "world-edits batch `{bid}` places gravity-affected `{}` at {:?} with no \
                     support anywhere below — it would fall out of the void world and despawn, \
                     silently deforming the map. Give it a substrate (fill solid blocks below \
                     it first) or use a non-falling block; do NOT keep the edit and hope the \
                     hole is harmless",
                    lost.block, lost.from
                ),
            });
        }

        // Post-batch invariants: relight re-entry (spec-0010), walkability
        // re-proof, boundary safety (spec-0017). Each failure names the batch.
        if enforce {
            check_batch_invariants(plan, &assembled, bid)?;
        }

        // Runtime materialization + snapshot bounds for this batch.
        commands.extend(coalesce_commands(&batch_writes));
        batches.push(BatchOutcome {
            id: bid.to_string(),
            bounds: bounds_of(batch_writes.keys()),
        });
    }

    Ok(Some(EditReplay {
        assembled,
        commands,
        batches,
    }))
}

/// Re-prove the post-edit invariants over the current assembled world, naming
/// `bid` in any failure. Mirrors the final build's pass order: relight first
/// (its colliding fixtures join the nav world), then the walkability proofs,
/// then boundary safety.
fn check_batch_invariants(plan: &Plan, assembled: &Assembled, bid: &str) -> Result<(), EditError> {
    let relight = crate::light::relight_over(plan, assembled);
    if let Some(diag) = relight.diagnostics.first() {
        return Err(EditError {
            code: diag.code,
            message: format!("after world-edits batch `{bid}`: {}", diag.message),
        });
    }
    let world = crate::nav::World::from_occupancy(assembled::occupancy_of(
        assembled.blocks.clone(),
        &assembled.open_gates,
    ));
    let with_fixtures = if relight.extra_solid.is_empty() {
        world
    } else {
        let mut occ = assembled::occupancy_of(assembled.blocks.clone(), &assembled.open_gates);
        occ.solid.extend(relight.extra_solid.iter().copied());
        crate::nav::World::from_occupancy(occ)
    };
    let ctx = |e: crate::nav::NavError| EditError {
        code: e.code,
        message: format!("after world-edits batch `{bid}`: {}", e.message),
    };
    if crate::nav::needs_world(plan) {
        crate::nav::check_critical_path(plan, &with_fixtures).map_err(ctx)?;
        crate::nav::check_checkpoints(plan, &with_fixtures).map_err(ctx)?;
    }
    let starts = anchor_starts(plan);
    crate::nav::verify_boundary_safety(&with_fixtures, &starts).map_err(ctx)?;
    Ok(())
}

/// Every resolved anchor position of the plan — the same reachability roots the
/// relight pass floods from (a `Point`'s cell; a `Gate`'s `from` corner).
pub fn anchor_starts(plan: &Plan) -> Vec<[i32; 3]> {
    plan.anchors
        .values()
        .map(|resolved| match resolved {
            ResolvedAnchor::Point { pos, .. } => *pos,
            ResolvedAnchor::Gate { from, .. } => *from,
        })
        .collect()
}

/// Write one cell: the assembled model stores the **base** block id (the
/// classification helpers match exact names), the batch write-log keeps the
/// full blockstate-carrying form for runtime `setblock`/`fill` emission. An
/// air write removes the cell (absent = air) and always clears any open-gate
/// marking; a non-air write over an authored open gate likewise closes it
/// (the runtime `setblock` replaces the whole block, state included).
fn write_cell(
    assembled: &mut Assembled,
    batch_writes: &mut BTreeMap<[i32; 3], String>,
    cell: [i32; 3],
    block: &str,
) {
    let base = base_id(block);
    if assembled::is_air(base) {
        assembled.blocks.remove(&cell);
    } else {
        assembled.blocks.insert(cell, base.to_string());
    }
    assembled.open_gates.remove(&cell);
    batch_writes.insert(cell, block.to_string());
}

/// Look up a verb's target region, rejecting an empty one: an edit that touches
/// zero cells is a silent no-op — always a defect (the region drifted off the
/// content it targeted), never something to ship quietly.
fn used_region<'r>(
    bid: &str,
    regions: &'r BTreeMap<String, BTreeSet<[i32; 3]>>,
    name: &str,
) -> Result<&'r BTreeSet<[i32; 3]>, EditError> {
    let cells = regions.get(name).ok_or_else(|| EditError {
        code: DW_EDIT_UNRESOLVED,
        message: format!(
            "world-edits batch `{bid}` uses region `{name}` before any `select` defines it \
             (validation should have caught this)"
        ),
    })?;
    if cells.is_empty() {
        return Err(EditError {
            code: DW_EDIT_UNRESOLVED,
            message: format!(
                "world-edits batch `{bid}`: region `{name}` resolves to zero cells, so this \
                 edit would be a silent no-op — its palette-match/composition no longer \
                 matches the world it targeted. Re-check the region against a fresh \
                 `delvec snapshot` and fix the select; do NOT leave a dead edit in the script"
            ),
        });
    }
    Ok(cells)
}

/// Resolve a `select` shape to world cells against the solved layout and the
/// current (mid-replay) world state. Errors are `DW0323` messages.
fn resolve_shape(
    plan: &Plan,
    area: &crate::plan::AreaPlacement,
    bid: &str,
    regions: &BTreeMap<String, BTreeSet<[i32; 3]>>,
    assembled: &Assembled,
    shape: &RegionShape,
) -> Result<BTreeSet<[i32; 3]>, String> {
    let named = |name: &str| -> Result<&BTreeSet<[i32; 3]>, String> {
        regions.get(name).ok_or_else(|| {
            format!(
                "world-edits batch `{bid}` composes region `{name}` before any `select` \
                 defines it (validation should have caught this)"
            )
        })
    };
    match shape {
        RegionShape::Box { frame, min, max } => {
            let (a, b) = match frame {
                EditFrame::PieceLocal { piece, prefab } => {
                    let idx = *piece as usize;
                    let placed = area.pieces.get(idx).ok_or_else(|| {
                        format!(
                            "world-edits batch `{bid}`: piece index {idx} is out of range — \
                             area `{}` placed {} piece(s) (0..={}). The layout this frame was \
                             authored against has drifted; re-inspect it with `delvec \
                             snapshot` and update the frame, do NOT guess an index",
                            area.area_id,
                            area.pieces.len(),
                            area.pieces.len().saturating_sub(1),
                        )
                    })?;
                    if placed.prefab_id != prefab.as_str() {
                        return Err(format!(
                            "world-edits batch `{bid}`: piece {idx} of area `{}` is \
                             `{}`, not the frame's declared `{prefab}` — the solved layout \
                             has drifted since this edit was authored. Re-inspect the layout \
                             (`delvec snapshot`) and re-target the frame; do NOT just delete \
                             the prefab guard",
                            area.area_id, placed.prefab_id,
                        ));
                    }
                    let t1 = placed.rotation.transform(*min);
                    let t2 = placed.rotation.transform(*max);
                    (
                        [
                            placed.pos[0] + t1[0],
                            placed.pos[1] + t1[1],
                            placed.pos[2] + t1[2],
                        ],
                        [
                            placed.pos[0] + t2[0],
                            placed.pos[1] + t2[1],
                            placed.pos[2] + t2[2],
                        ],
                    )
                }
                EditFrame::AnchorRelative { anchor } => {
                    let key = (area.area_id.clone(), anchor.as_str().to_string());
                    let pos = match plan.anchors.get(&key) {
                        Some(ResolvedAnchor::Point { pos, .. }) => *pos,
                        Some(ResolvedAnchor::Gate { from, .. }) => *from,
                        None => {
                            return Err(format!(
                                "world-edits batch `{bid}`: area `{}` resolves no anchor \
                                 `{anchor}` — use an anchor the area's placed prefab metadata \
                                 declares (see the `delvec snapshot` manifest's target list)",
                                area.area_id,
                            ));
                        }
                    };
                    (
                        [pos[0] + min[0], pos[1] + min[1], pos[2] + min[2]],
                        [pos[0] + max[0], pos[1] + max[1], pos[2] + max[2]],
                    )
                }
            };
            Ok(assembled::region_cells(a, b).collect())
        }
        RegionShape::SurfaceBand { over, from, to } => {
            let base = named(over.as_str())?;
            let mut out = BTreeSet::new();
            for ((x, z), ys) in columns(base) {
                let Some(surf) = ys
                    .iter()
                    .copied()
                    .filter(|&y| assembled.blocks.contains_key(&[x, y, z]))
                    .max()
                else {
                    continue; // an all-air column has no surface
                };
                for y in (surf + from)..=(surf + to) {
                    out.insert([x, y, z]);
                }
            }
            Ok(out)
        }
        RegionShape::PaletteMatch { within, blocks } => {
            let base = named(within.as_str())?;
            let matches: BTreeSet<&str> = blocks.iter().map(|m| strip_ns(base_id(m))).collect();
            Ok(base
                .iter()
                .filter(|cell| {
                    assembled
                        .blocks
                        .get(*cell)
                        .is_some_and(|name| matches.contains(strip_ns(name.as_str())))
                })
                .copied()
                .collect())
        }
        RegionShape::Union { of } => {
            let mut out = BTreeSet::new();
            for r in of {
                out.extend(named(r.as_str())?.iter().copied());
            }
            Ok(out)
        }
        RegionShape::Intersect { of } => {
            let mut iter = of.iter();
            let first = iter.next().ok_or_else(|| {
                format!("world-edits batch `{bid}`: empty intersection (validated earlier)")
            })?;
            let mut out = named(first.as_str())?.clone();
            for r in iter {
                let next = named(r.as_str())?;
                out.retain(|c| next.contains(c));
            }
            Ok(out)
        }
        RegionShape::Subtract { base, remove } => {
            let mut out = named(base.as_str())?.clone();
            for r in remove {
                let gone = named(r.as_str())?;
                out.retain(|c| !gone.contains(c));
            }
            Ok(out)
        }
    }
}

/// Apply a `morph` op to the region's columns. The region defines the
/// **footprint** (its columns) and where the current surface is read (the
/// highest occupied cell within the region's y-range per column); `raise` and
/// `smooth` may write **above** the region's top — reshaping upward is the
/// verb's purpose (berm → natural slope) — while removal only ever touches
/// occupied cells at or below the read surface. Every write is computed from
/// the pre-op surface in one deterministic scan (`BTreeMap` order), so column
/// order can never affect the result.
fn morph(
    assembled: &mut Assembled,
    batch_writes: &mut BTreeMap<[i32; 3], String>,
    cells: &BTreeSet<[i32; 3]>,
    op: &MorphOp,
    seed: u64,
) {
    let cols = columns(cells);
    // Each column's surface: the highest occupied cell within the region column,
    // or None for an all-air column (which never participates).
    let surface = |assembled: &Assembled, x: i32, z: i32, ys: &[i32]| -> Option<i32> {
        ys.iter()
            .copied()
            .filter(|&y| assembled.blocks.contains_key(&[x, y, z]))
            .max()
    };
    // Set one column's surface to `target`, adding recipe cells above the old
    // surface or carving occupied cells down to it.
    fn set_surface(
        assembled: &mut Assembled,
        batch_writes: &mut BTreeMap<[i32; 3], String>,
        x: i32,
        z: i32,
        surf: i32,
        target: i32,
        recipe: Option<(&PaletteRecipe, u64)>,
    ) {
        if target > surf {
            let (recipe, seed) = recipe.expect("raising morphs carry a recipe");
            for y in (surf + 1)..=target {
                let cell = [x, y, z];
                let block = pick(recipe, noise_at(seed, cell, recipe));
                write_cell(assembled, batch_writes, cell, block);
            }
        } else {
            for y in (target + 1)..=surf {
                if assembled.blocks.contains_key(&[x, y, z]) {
                    write_cell(assembled, batch_writes, [x, y, z], "minecraft:air");
                }
            }
        }
    }
    match op {
        MorphOp::Raise { by, recipe } => {
            for ((x, z), ys) in &cols {
                let Some(surf) = surface(assembled, *x, *z, ys) else {
                    continue;
                };
                let target = surf + *by as i32;
                set_surface(
                    assembled,
                    batch_writes,
                    *x,
                    *z,
                    surf,
                    target,
                    Some((recipe, seed)),
                );
            }
        }
        MorphOp::Lower { by } => {
            for ((x, z), ys) in &cols {
                // Remove the top `by` OCCUPIED cells of the region column (a gap
                // under the surface does not shield the cells below it).
                let mut occupied: Vec<i32> = ys
                    .iter()
                    .copied()
                    .filter(|&y| assembled.blocks.contains_key(&[*x, y, *z]))
                    .collect();
                occupied.sort_unstable();
                for &y in occupied.iter().rev().take(*by as usize) {
                    write_cell(assembled, batch_writes, [*x, y, *z], "minecraft:air");
                }
            }
        }
        MorphOp::Smooth { passes, recipe } => {
            // Height field over the region's columns, double buffered; columns
            // outside the region do not participate. Each pass relaxes a column
            // toward the round-half-up mean of itself + its cardinal neighbours,
            // clamped to ±1 per pass (a smooth is a relaxation, never a jump).
            let mut heights: BTreeMap<(i32, i32), i32> = cols
                .iter()
                .filter_map(|((x, z), ys)| surface(assembled, *x, *z, ys).map(|s| ((*x, *z), s)))
                .collect();
            let start = heights.clone();
            for _ in 0..*passes {
                let prev = heights.clone();
                for ((x, z), h) in heights.iter_mut() {
                    let mut sum = *h as i64;
                    let mut n = 1i64;
                    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        if let Some(nh) = prev.get(&(x + dx, z + dz)) {
                            sum += *nh as i64;
                            n += 1;
                        }
                    }
                    let mean = ((2 * sum + n) / (2 * n)) as i32;
                    *h += (mean - *h).clamp(-1, 1);
                }
            }
            for ((x, z), target) in heights {
                let surf = start[&(x, z)];
                if target != surf {
                    set_surface(
                        assembled,
                        batch_writes,
                        x,
                        z,
                        surf,
                        target,
                        Some((recipe, seed)),
                    );
                }
            }
        }
    }
}

/// Group region cells into columns: `(x, z)` → ascending `y`s.
fn columns(cells: &BTreeSet<[i32; 3]>) -> BTreeMap<(i32, i32), Vec<i32>> {
    let mut cols: BTreeMap<(i32, i32), Vec<i32>> = BTreeMap::new();
    for c in cells {
        cols.entry((c[0], c[2])).or_default().push(c[1]);
    }
    for ys in cols.values_mut() {
        ys.sort_unstable();
    }
    cols
}

/// The union AABB of an iterator of cells.
fn bounds_of<'c>(cells: impl Iterator<Item = &'c [i32; 3]>) -> Option<([i32; 3], [i32; 3])> {
    let mut min = [i32::MAX; 3];
    let mut max = [i32::MIN; 3];
    let mut any = false;
    for c in cells {
        any = true;
        for a in 0..3 {
            min[a] = min[a].min(c[a]);
            max[a] = max[a].max(c[a]);
        }
    }
    any.then_some((min, max))
}

/// Lower a batch's writes to vanilla commands: consecutive same-block runs
/// along x at fixed `(y, z)` coalesce into one `fill`, everything else is a
/// `setblock`. `BTreeMap` iteration order keys on `[x, y, z]`, so re-sort into
/// `(y, z, x)` scan order first for maximal runs. Deterministic.
fn coalesce_commands(writes: &BTreeMap<[i32; 3], String>) -> Vec<String> {
    let mut ordered: Vec<(&[i32; 3], &String)> = writes.iter().collect();
    ordered.sort_by_key(|(c, _)| (c[1], c[2], c[0]));
    let mut out = Vec::new();
    let mut i = 0;
    while i < ordered.len() {
        let (start, block) = ordered[i];
        let mut end = start[0];
        let mut j = i + 1;
        while j < ordered.len() {
            let (next, nblock) = ordered[j];
            if next[1] == start[1] && next[2] == start[2] && next[0] == end + 1 && nblock == block {
                end = next[0];
                j += 1;
            } else {
                break;
            }
        }
        if end > start[0] {
            out.push(format!(
                "fill {} {} {} {} {} {} {block}",
                start[0], start[1], start[2], end, start[1], start[2]
            ));
        } else {
            out.push(format!(
                "setblock {} {} {} {block}",
                start[0], start[1], start[2]
            ));
        }
        i = j;
    }
    out
}

// ---------------------------------------------------------------------------
// The seeded palette primitive family (ported from the island/cave prefab
// generators — `prefabs/cave-generator`, `prefabs/island-terrain-generator` —
// the repo's proven "palette recipe + value noise" idiom: a smooth noise field
// keys a cumulative-weight palette, so picks cluster into strata/patches
// instead of per-cell speckle).
// ---------------------------------------------------------------------------

/// The default noise frequency (blocks⁻¹) when a recipe declares no `scale`.
const DEFAULT_NOISE_SCALE: f64 = 0.35;

/// splitmix64 finalizer (the generators' `mix64`).
fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// White noise in `[0, 1)` at an integer lattice point (the generators'
/// `hash01`, verbatim).
fn hash01(seed: u64, x: i32, y: i32, z: i32, salt: u64) -> f64 {
    let mut h = seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h = mix64(h ^ (x as i64 as u64).wrapping_mul(0x0000_0100_0000_01B3));
    h = mix64(h ^ (y as i64 as u64).wrapping_mul(0xFF51_AFD7_ED55_8CCD));
    h = mix64(h ^ (z as i64 as u64).wrapping_mul(0xC4CE_B9FE_1A85_EC53));
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// Smoothstep fade.
fn fade(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// Linear interpolation.
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Trilinearly-interpolated value noise in `[0, 1]` — smooth, so palette picks
/// cluster into patches (the generators' `value_noise`, verbatim).
fn value_noise(seed: u64, x: i32, y: i32, z: i32, freq: f64, salt: u64) -> f64 {
    let (fx, fy, fz) = (x as f64 * freq, y as f64 * freq, z as f64 * freq);
    let (x0, y0, z0) = (fx.floor(), fy.floor(), fz.floor());
    let (tx, ty, tz) = (fade(fx - x0), fade(fy - y0), fade(fz - z0));
    let (ix, iy, iz) = (x0 as i32, y0 as i32, z0 as i32);
    let c = |dx: i32, dy: i32, dz: i32| hash01(seed, ix + dx, iy + dy, iz + dz, salt);
    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), tx);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), tx);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), tx);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), tx);
    let y0i = lerp(x00, x10, ty);
    let y1i = lerp(x01, x11, ty);
    lerp(y0i, y1i, tz)
}

/// The recipe's noise sample at a cell.
fn noise_at(seed: u64, cell: [i32; 3], recipe: &PaletteRecipe) -> f64 {
    let freq = recipe.scale.unwrap_or(DEFAULT_NOISE_SCALE);
    value_noise(seed, cell[0], cell[1], cell[2], freq, 0)
}

/// Pick a palette entry by cumulative weight at noise sample `n` (the
/// generators' `pick`). `n` is clamped just below 1 so the last entry's band is
/// inclusive.
fn pick(recipe: &PaletteRecipe, n: f64) -> &str {
    let total: f64 = recipe.blocks.iter().map(|b| b.weight).sum();
    let mut t = n.min(0.999_999) * total;
    for b in &recipe.blocks {
        if t < b.weight {
            return &b.block;
        }
        t -= b.weight;
    }
    &recipe
        .blocks
        .last()
        .expect("validated: recipe has ≥ 1 entry")
        .block
}

/// A block field's base id: everything before a `[state]` suffix.
fn base_id(block: &str) -> &str {
    match block.find('[') {
        Some(open) => &block[..open],
        None => block,
    }
}

/// Strip the `minecraft:` namespace for base-id comparison.
fn strip_ns(id: &str) -> &str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use delvewright_dsl::PaletteBlock;

    fn recipe(entries: &[(&str, f64)]) -> PaletteRecipe {
        PaletteRecipe {
            blocks: entries
                .iter()
                .map(|(b, w)| PaletteBlock {
                    block: (*b).to_string(),
                    weight: *w,
                })
                .collect(),
            scale: None,
        }
    }

    #[test]
    fn value_noise_is_deterministic_and_bounded() {
        for cell in [[0, 0, 0], [17, -3, 250], [-40, 64, 9]] {
            let a = value_noise(42, cell[0], cell[1], cell[2], 0.35, 0);
            let b = value_noise(42, cell[0], cell[1], cell[2], 0.35, 0);
            assert_eq!(a.to_bits(), b.to_bits(), "same inputs → same bits");
            assert!((0.0..=1.0).contains(&a));
        }
        // A different seed decorrelates.
        assert_ne!(
            value_noise(42, 5, 5, 5, 0.35, 0).to_bits(),
            value_noise(43, 5, 5, 5, 0.35, 0).to_bits()
        );
    }

    #[test]
    fn pick_partitions_by_cumulative_weight() {
        let r = recipe(&[
            ("minecraft:stone", 3.0),
            ("minecraft:mossy_cobblestone", 1.0),
        ]);
        assert_eq!(pick(&r, 0.0), "minecraft:stone");
        assert_eq!(pick(&r, 0.74), "minecraft:stone");
        assert_eq!(pick(&r, 0.76), "minecraft:mossy_cobblestone");
        assert_eq!(pick(&r, 1.0), "minecraft:mossy_cobblestone");
    }

    #[test]
    fn base_id_and_ns_stripping() {
        assert_eq!(
            base_id("minecraft:oak_leaves[persistent=true]"),
            "minecraft:oak_leaves"
        );
        assert_eq!(base_id("minecraft:stone"), "minecraft:stone");
        assert_eq!(strip_ns("minecraft:stone"), "stone");
        assert_eq!(strip_ns("stone"), "stone");
    }

    #[test]
    fn coalesce_merges_x_runs_only() {
        let mut writes: BTreeMap<[i32; 3], String> = BTreeMap::new();
        for x in 0..4 {
            writes.insert([x, 64, 0], "minecraft:stone".to_string());
        }
        writes.insert([6, 64, 0], "minecraft:stone".to_string());
        writes.insert([0, 65, 0], "minecraft:air".to_string());
        let cmds = coalesce_commands(&writes);
        assert_eq!(
            cmds,
            vec![
                "fill 0 64 0 3 64 0 minecraft:stone".to_string(),
                "setblock 6 64 0 minecraft:stone".to_string(),
                "setblock 0 65 0 minecraft:air".to_string(),
            ]
        );
    }
}
