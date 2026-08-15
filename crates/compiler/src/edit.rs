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
//! 5. **Trap-hardware integrity** (`DW0352`): no batch write may land on a
//!    trap's trigger / dispenser / disarm cell. `setup_finish` runs
//!    `world_edits` *before* `trap_setup`, so such an edit silently kills the
//!    trap at runtime (`item replace block … container.0` into a non-container
//!    fails with no output) while every other proof stays green.
//!
//! And two advisory (warning-tier) checks, reported but never fatal:
//!
//! * **Gate-region collision** (`DW0353`): a write inside a `close-gate`
//!   region survives the proofs but is visually destroyed by one close/open
//!   cycle (the gate fill overwrites it, the open clears it to air).
//! * **Support validity** (`DW0354`): a support-dependent block (torch,
//!   flora, …) whose support a later batch removed, or flora scattered onto a
//!   block flowers cannot stand on. Error tier when the broken block is a
//!   fixture the script's own `relight` verb placed — that one is a *declared*
//!   lighting guarantee, not decoration.
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

use delvewright_dsl::{Diagnostic, EditFrame, MorphOp, PaletteRecipe, RegionShape, WorldEdit};

use crate::assembled::{self, Assembled};
use crate::plan::{Plan, ResolvedAnchor};
use crate::solver::stream_seed;
use delvewright_dsl::DwCode;

/// A stage-7 edit's frame or region fails to resolve against the solved layout:
/// a piece index out of range, a piece whose prefab differs from the frame's
/// declared one (layout drift), an anchor the area does not resolve, or a verb
/// whose target region resolves to zero cells (a silent no-op is always a
/// defect). Build-tier (exit 3).
pub const DW_EDIT_UNRESOLVED: DwCode = DwCode::every_version("DW0323");

/// A stage-7 batch writes a block into a cell a trap's hardware occupies (its
/// trigger/hazard cell, its dispenser socket, or its disarm affordance cell).
/// `setup_finish` applies `world_edits` **before** `trap_setup`, so the edit
/// wins and the trap is loaded into a block that is no longer there — vanilla's
/// `item replace block … container.0` on a non-container fails with no output,
/// shipping a dead trap past every green proof. Build-tier (exit 3).
pub const DW_EDIT_TRAP_HARDWARE: DwCode = DwCode::every_version("DW0352");

/// **Advisory.** A stage-7 batch writes inside a `close-gate` region: the
/// gameplay seal fills that region with the gate anchor's block and `open-gate`
/// clears it to air, so one close/open cycle destroys the edit visually. Every
/// proof stays sound (the occupancy model already treats the region as
/// gate-controlled), so this warns rather than rejects.
pub const DW_EDIT_GATE_REGION: DwCode = DwCode::every_version("DW0353");

/// A support-dependent block (torch, lantern, flora, …) the edit script placed
/// has no valid support in the post-batch world — its support block was carved
/// or replaced by a later batch, or flora was scattered onto a block flowers
/// cannot stand on. Vanilla pops such a block off as an item the moment the
/// chunk ticks, silently undoing the edit. **Advisory** for decoration;
/// **error** when the popped block is a fixture the script's own `relight` verb
/// placed (a declared minimum-light guarantee, not decoration).
pub const DW_EDIT_SUPPORT: DwCode = DwCode::every_version("DW0354");

/// A failed edit replay: a stable diagnostic code plus a message that names the
/// offending batch.
#[derive(Debug)]
pub struct EditError {
    /// Stable `DW####` code (may be a reused invariant code, e.g. `DW0311`).
    pub code: DwCode,
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
    /// Advisory findings (`DW0353`/`DW0354`) raised during the replay, in batch
    /// order. Never fatal — `delvec` exits non-zero only on `Severity::Error`.
    /// Empty for a view replay (`replay_view` proves nothing).
    pub warnings: Vec<Diagnostic>,
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
    prefabs: &crate::registry::PrefabRegistry,
    structures: &BTreeMap<String, Vec<u8>>,
) -> Result<Option<EditReplay>, EditError> {
    replay_with(plan, prefabs, structures, true)
}

/// [`replay`] for **view** commands (`delvec snapshot` / `blocking-chart`): the
/// edits are applied and gravity re-settles, but the invariant re-proofs are
/// skipped — a view command must be able to SHOW a world state whose invariants
/// fail (that is exactly what the author needs to look at). Region-resolution
/// failures still error: an unresolvable edit has no world state to show.
pub fn replay_view(
    plan: &Plan,
    prefabs: &crate::registry::PrefabRegistry,
    structures: &BTreeMap<String, Vec<u8>>,
) -> Result<Option<EditReplay>, EditError> {
    replay_with(plan, prefabs, structures, false)
}

fn replay_with(
    plan: &Plan,
    prefabs: &crate::registry::PrefabRegistry,
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
    let mut warnings: Vec<Diagnostic> = Vec::new();
    // Support-validity bookkeeping (DW0354), cumulative across batches: every
    // support-dependent block the script has placed so far, and which of those
    // cells a `relight` verb placed (those are declared lighting, error tier).
    let mut support_watch: BTreeMap<[i32; 3], String> = BTreeMap::new();
    let mut fixture_cells: BTreeSet<[i32; 3]> = BTreeSet::new();

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
                    check_recipe(bid, "fill", recipe)?;
                    for &cell in cells {
                        let block = pick(recipe, noise_at(seed, cell, recipe))
                            .ok_or_else(|| empty_recipe(bid, "fill"))?;
                        write_cell(&mut assembled, &mut batch_writes, cell, block);
                    }
                }
                WorldEdit::Replace {
                    region,
                    matching,
                    recipe,
                } => {
                    let cells = used_region(bid, &regions, region.as_str())?;
                    check_recipe(bid, "replace", recipe)?;
                    let matches: BTreeSet<&str> =
                        matching.iter().map(|m| strip_ns(base_id(m))).collect();
                    for &cell in cells {
                        let Some(current) = assembled.blocks.get(&cell) else {
                            continue; // air never matches a base id
                        };
                        if !matches.contains(strip_ns(base_id(current))) {
                            continue;
                        }
                        let block = pick(recipe, noise_at(seed, cell, recipe))
                            .ok_or_else(|| empty_recipe(bid, "replace"))?;
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
                    morph(&mut assembled, &mut batch_writes, cells, op, seed, bid)?;
                }
                WorldEdit::Scatter {
                    region,
                    items,
                    density,
                    avoid,
                    spacing,
                    limit,
                } => {
                    let cells = used_region(bid, &regions, region.as_str())?.clone();
                    let avoid_fp = avoid_footprint(bid, &regions, avoid)?;
                    scatter(
                        &mut assembled,
                        &mut batch_writes,
                        &cells,
                        &avoid_fp,
                        items,
                        *density,
                        *spacing,
                        *limit,
                        seed,
                        bid,
                    )?;
                }
                WorldEdit::Plant {
                    region,
                    tree,
                    count,
                    avoid,
                    spacing,
                } => {
                    let cells = used_region(bid, &regions, region.as_str())?.clone();
                    let avoid_fp = avoid_footprint(bid, &regions, avoid)?;
                    plant(
                        &mut assembled,
                        &mut batch_writes,
                        &cells,
                        &avoid_fp,
                        *tree,
                        *count,
                        spacing.unwrap_or(4),
                        seed,
                    );
                }
                WorldEdit::Fragment {
                    prefab,
                    frame,
                    at,
                    rotation,
                } => {
                    fragment(
                        &mut assembled,
                        &mut batch_writes,
                        plan,
                        area,
                        bid,
                        prefabs,
                        structures,
                        prefab,
                        frame,
                        *at,
                        rotation.unwrap_or(delvewright_dsl::FragmentRotation::None),
                    )?;
                }
                WorldEdit::Relight {
                    region,
                    fixture,
                    min_light,
                } => {
                    let cells = used_region(bid, &regions, region.as_str())?.clone();
                    relight_region(
                        &mut assembled,
                        &mut batch_writes,
                        &mut fixture_cells,
                        plan,
                        batch,
                        bid,
                        &cells,
                        *fixture,
                        *min_light,
                    )?;
                }
                // L2 massing verbs (spec-0017) were applied at PLAN time
                // (`crate::massing`, inside `Plan::build`) — the assembly this
                // replay started from already reflects them. Their batch
                // bounds come from `plan.massing_bounds` below.
                WorldEdit::SwapPiece { .. }
                | WorldEdit::InsertPiece { .. }
                | WorldEdit::RemovePiece { .. }
                | WorldEdit::RewireSocket { .. }
                | WorldEdit::ReseedPiece { .. } => {}
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

        // Every support-dependent block this batch placed joins the cumulative
        // watch list; a later batch that carves its support is what DW0354
        // catches. Recorded regardless of `enforce` (cheap, and a view replay
        // simply never evaluates it).
        for (cell, block) in &batch_writes {
            if support_of(block).is_some() {
                support_watch.insert(*cell, block.clone());
            }
        }

        // Post-batch invariants: trap-hardware integrity (spec-0017 audit),
        // relight re-entry (spec-0010), walkability re-proof, boundary safety
        // (spec-0017). Each failure names the batch.
        if enforce {
            check_batch_invariants(plan, &assembled, bid, &batch_writes)?;
            check_support(
                &assembled,
                bid,
                &support_watch,
                &fixture_cells,
                &mut warnings,
            )?;
            warnings.extend(gate_region_warnings(plan, bid, &batch_writes));
        }

        // Runtime materialization + snapshot bounds for this batch.
        commands.extend(coalesce_commands(&batch_writes));
        batches.push(BatchOutcome {
            id: bid.to_string(),
            bounds: bounds_of(batch_writes.keys())
                .or_else(|| plan.massing_bounds.get(bid).copied()),
        });
    }

    Ok(Some(EditReplay {
        assembled,
        commands,
        batches,
        warnings,
    }))
}

/// Re-prove the post-edit invariants over the current assembled world, naming
/// `bid` in any failure. Mirrors the final build's pass order: relight first
/// (its colliding fixtures join the nav world), then the walkability proofs,
/// then boundary safety.
fn check_batch_invariants(
    plan: &Plan,
    assembled: &Assembled,
    bid: &str,
    batch_writes: &BTreeMap<[i32; 3], String>,
) -> Result<(), EditError> {
    // Trap-hardware integrity first: it is a *structural* clash the geometry
    // proofs below can never see (they model walkability and light, not whether
    // a dispenser is still a dispenser).
    check_trap_hardware(plan, bid, batch_writes)?;
    let relight = crate::light::relight_over(plan, assembled);
    if let Some(diag) = relight.diagnostics.first() {
        return Err(EditError {
            code: diag.code,
            message: format!("after world-edits batch `{bid}`: {}", diag.message),
        });
    }
    // The world-generator ambient (spec-0013 `horizon`) is a *premise* of the
    // boundary-safety proof, not geometry: under `ocean` the pinned superflat
    // puts bedrock under every column, so there is no void to step into and the
    // hazard is stranding instead (`nav::verify_boundary_safety`). Deriving it
    // from the plan here is what keeps that proof from testing a false premise.
    let ambient = crate::nav::Ambient::of_plan(plan);
    let world = crate::nav::World::from_occupancy(assembled::occupancy_of(
        assembled.blocks.clone(),
        &assembled.open_gates,
    ))
    .with_ambient(ambient.clone());
    let with_fixtures = if relight.extra_solid.is_empty() {
        world
    } else {
        let mut occ = assembled::occupancy_of(assembled.blocks.clone(), &assembled.open_gates);
        occ.solid.extend(relight.extra_solid.iter().copied());
        crate::nav::World::from_occupancy(occ).with_ambient(ambient)
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

/// Every cell whose **block** a trap's hardware depends on, mapped to
/// `(trap id, role)`. Content order (`plan.traps`) decides ties, so the message
/// a colliding edit gets is deterministic.
fn trap_hardware<'p>(plan: &'p Plan) -> BTreeMap<[i32; 3], (&'p str, &'static str)> {
    let mut out: BTreeMap<[i32; 3], (&'p str, &'static str)> = BTreeMap::new();
    for t in &plan.traps {
        out.entry(t.trigger_cell)
            .or_insert((t.id.as_str(), "trigger/hazard"));
        if let Some(d) = t.dispenser {
            out.entry(d).or_insert((t.id.as_str(), "dispenser socket"));
        }
        if let Some(dis) = &t.disarm {
            out.entry(dis.via_cell)
                .or_insert((t.id.as_str(), "disarm affordance"));
        }
    }
    out
}

/// **Trap-hardware integrity** (`DW0352`, map-editor audit). The runtime order
/// inside `setup_finish` is `world_edits` → … → `trap_setup`: an edit that
/// filled or carved a trap's trigger, dispenser socket or disarm cell lands
/// FIRST, and `trap_setup`'s `item replace block … container.0` then addresses
/// a block that is no longer a container — which vanilla fails **silently**.
/// The trap ships dead, its `DW0342` completability proof still green (that
/// proof reasons about the *planned* hazard, not the surviving hardware). No
/// geometry proof can see this, so it gets its own structural check.
fn check_trap_hardware(
    plan: &Plan,
    bid: &str,
    batch_writes: &BTreeMap<[i32; 3], String>,
) -> Result<(), EditError> {
    if plan.traps.is_empty() {
        return Ok(());
    }
    let hardware = trap_hardware(plan);
    for (cell, block) in batch_writes {
        let Some((trap, role)) = hardware.get(cell) else {
            continue;
        };
        return Err(EditError {
            code: DW_EDIT_TRAP_HARDWARE,
            message: format!(
                "world-edits batch `{bid}` writes `{block}` at [{}, {}, {}] — that cell is trap \
                 `{trap}`'s {role}. `setup_finish` runs `world_edits` BEFORE `trap_setup`, so \
                 this edit lands first and the trap is then wired into a block that is no \
                 longer there: `item replace block … container.0` on a non-container fails \
                 SILENTLY, and the delve ships a dead trap with every proof green (`DW0342` \
                 proves the planned hazard, not the surviving hardware). Move the region off \
                 the trap's trigger/dispenser/disarm cells, or re-anchor the trap; do NOT \
                 assume the edit leaves the redstone intact",
                cell[0], cell[1], cell[2]
            ),
        });
    }
    Ok(())
}

/// **Gate-region collision** (`DW0353`, advisory). A `close-gate` fills its
/// region with the gate anchor's declared block and `open-gate` clears it back
/// to air, so anything a batch wrote inside that region is destroyed by the
/// first close/open cycle. The proofs stay sound — the occupancy model already
/// treats gate regions as gate-controlled — so this warns: an author may
/// legitimately be dressing the *closed* state.
fn gate_region_warnings(
    plan: &Plan,
    bid: &str,
    batch_writes: &BTreeMap<[i32; 3], String>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for ev in &plan.region_events {
        if !ev.fills() {
            continue;
        }
        let (lo, hi) = ev.region;
        let hits: Vec<[i32; 3]> = batch_writes
            .keys()
            .copied()
            .filter(|c| (0..3).all(|a| c[a] >= lo[a] && c[a] <= hi[a]))
            .collect();
        let Some(first) = hits.first() else { continue };
        out.push(Diagnostic::warning(
            DW_EDIT_GATE_REGION,
            "world-edits",
            format!("/content/batches/{bid}"),
            format!(
                "world-edits batch `{bid}` writes {} cell(s) (first [{}, {}, {}]) inside a \
                 `close-gate` region [{}, {}, {}]..[{}, {}, {}]. That region is filled solid \
                 when the gate closes and cleared to AIR when it opens, so one close/open \
                 cycle erases the edit — the dressing you see in `delvec snapshot` is not what \
                 players see after the beat fires. Move the edit outside the gate region, or \
                 keep it only if you are deliberately dressing the sealed state",
                hits.len(),
                first[0],
                first[1],
                first[2],
                lo[0],
                lo[1],
                lo[2],
                hi[0],
                hi[1],
                hi[2],
            ),
        ));
        // One finding per gate region is enough: the remedy is the same for
        // every cell, and a listing would be noise.
    }
    out
}

/// What a block needs underneath it to survive a chunk tick.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Support {
    /// Any full-support block below (torch, lantern, campfire, carpet, …).
    SolidBelow,
    /// A block flowers/grass can root in (dirt family, moss, farmland, mud).
    Soil,
}

/// Blocks that pop off without a solid block below them.
const NEEDS_SOLID_BELOW: &[&str] = &[
    "torch",
    "soul_torch",
    "redstone_torch",
    "lantern",
    "soul_lantern",
    "campfire",
    "soul_campfire",
    "candle",
    "flower_pot",
    "snow",
    "sea_pickle",
    "repeater",
    "comparator",
    "redstone_wire",
    "rail",
    "powered_rail",
    "detector_rail",
    "activator_rail",
];

/// Blocks that pop off unless they are rooted in soil.
const NEEDS_SOIL: &[&str] = &[
    "poppy",
    "dandelion",
    "blue_orchid",
    "allium",
    "azure_bluet",
    "red_tulip",
    "orange_tulip",
    "white_tulip",
    "pink_tulip",
    "oxeye_daisy",
    "cornflower",
    "lily_of_the_valley",
    "wither_rose",
    "torchflower",
    "short_grass",
    "tall_grass",
    "fern",
    "large_fern",
    "oak_sapling",
    "spruce_sapling",
    "birch_sapling",
    "jungle_sapling",
    "acacia_sapling",
    "dark_oak_sapling",
    "cherry_sapling",
    "sweet_berry_bush",
];

/// Blocks flowers and grass root in.
const SOIL: &[&str] = &[
    "grass_block",
    "dirt",
    "coarse_dirt",
    "rooted_dirt",
    "podzol",
    "mycelium",
    "farmland",
    "moss_block",
    "mud",
    "muddy_mangrove_roots",
    "pale_moss_block",
];

/// The support a block field needs, or `None` when the block stands on its own
/// (or attaches sideways/above — `wall_torch`, a hanging lantern — where the
/// compiler models no support cell and must stay silent rather than guess).
fn support_of(block: &str) -> Option<Support> {
    if block.contains("hanging=true") {
        return None; // a hanging lantern's support is the block ABOVE it
    }
    let id = strip_ns(base_id(block));
    if NEEDS_SOIL.contains(&id) {
        Some(Support::Soil)
    } else if NEEDS_SOLID_BELOW.contains(&id) {
        Some(Support::SolidBelow)
    } else {
        None
    }
}

/// **Support validity** (`DW0354`) at batch close. Every support-dependent
/// block the script has placed so far is re-checked against the *current*
/// world: a later batch that carved the floor out from under a torch, or a
/// `scatter` that dropped flowers onto bare stone, leaves a block vanilla pops
/// off as an item the first time the chunk ticks — the edit silently undone,
/// with the compile-time model (which has no item-drop physics) still showing
/// it in every snapshot.
///
/// Advisory for decoration. **Error** when the popped block is a fixture the
/// script's own `relight` verb placed: that is a declared `min_light`
/// guarantee, and losing it re-darkens a region the `DW0211` proof passed.
/// Findings are aggregated per `(reason, block)` — a carved floor can strand
/// hundreds of flowers, and hundreds of identical lines are noise, not
/// information.
fn check_support(
    assembled: &Assembled,
    bid: &str,
    watch: &BTreeMap<[i32; 3], String>,
    fixture_cells: &BTreeSet<[i32; 3]>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(), EditError> {
    // (reason, block id) → (count, first offending cell)
    let mut agg: BTreeMap<(&'static str, String), (usize, [i32; 3])> = BTreeMap::new();
    for (cell, block) in watch {
        // A later batch overwrote this cell: it is no longer our placement.
        let base = strip_ns(base_id(block));
        match assembled.blocks.get(cell) {
            Some(current) if strip_ns(base_id(current)) == base => {}
            _ => continue,
        }
        let Some(need) = support_of(block) else {
            continue;
        };
        let below = [cell[0], cell[1] - 1, cell[2]];
        let under = assembled.blocks.get(&below);
        // `(singular, plural)` phrasing of the same finding: the fixture-tier
        // message names one block, the aggregated advisory names many.
        let reason = match (need, under) {
            (_, None) => (
                "has no block below it at all",
                "have no block below them at all",
            ),
            (Support::Soil, Some(u)) if !SOIL.contains(&strip_ns(base_id(u))) => (
                "sits on a block flowers cannot root in",
                "sit on a block flowers cannot root in",
            ),
            _ => continue,
        };
        // A broken relight fixture is a broken lighting guarantee: error now,
        // naming the cell precisely (there are never many).
        if fixture_cells.contains(cell) {
            return Err(EditError {
                code: DW_EDIT_SUPPORT,
                message: format!(
                    "after world-edits batch `{bid}`: the `relight` fixture `{block}` at \
                     [{}, {}, {}] {} — vanilla pops it off as an item the first time \
                     the chunk ticks, so the region silently loses the `min_light` the \
                     `DW0211` proof accepted. Keep the fixture's support intact (order the \
                     carving batch BEFORE the `relight`), or move the fixture; do NOT ship a \
                     light source the world drops on the floor",
                    cell[0], cell[1], cell[2], reason.0
                ),
            });
        }
        let entry = agg
            .entry((reason.1, base.to_string()))
            .or_insert((0, *cell));
        entry.0 += 1;
    }
    for ((reason, block), (count, first)) in agg {
        warnings.push(Diagnostic::warning(
            DW_EDIT_SUPPORT,
            "world-edits",
            format!("/content/batches/{bid}"),
            format!(
                "after world-edits batch `{bid}`: {count} placed `{block}` block(s) {reason} \
                 (first at [{}, {}, {}]). Vanilla pops a support-dependent block off \
                 as an item the first time the chunk ticks, so these writes silently vanish in \
                 the delivered world while every snapshot still shows them. Fix the support \
                 (put soil under flora, keep the floor under a torch) or reorder the batches \
                 so the carve happens first",
                first[0], first[1], first[2],
            ),
        ));
    }
    Ok(())
}

/// Every resolved anchor of the plan as a reachability root — the same roots the
/// relight pass floods from (a `Point`'s cell; a `Gate`'s `from` corner), each
/// carrying the AABB of the piece that DECLARES it.
///
/// The bounds are not decoration: seating an anchor is a nearest-standable-cell
/// snap, and an unconfined one walks through solid geometry (see
/// [`crate::nav::AnchorRoot`]). `plan.anchors` is a `BTreeMap`, so the order here
/// is deterministic (ADR-0006).
pub fn anchor_starts(plan: &Plan) -> Vec<crate::nav::AnchorRoot> {
    plan.anchors
        .iter()
        .map(|((area_id, _), resolved)| {
            let at = match resolved {
                ResolvedAnchor::Point { pos, .. } => *pos,
                ResolvedAnchor::Gate { from, .. } => *from,
            };
            crate::nav::AnchorRoot {
                at,
                within: plan.piece_bounds(area_id, at),
            }
        })
        .collect()
}

/// Write one cell: the assembled model and the batch write-log both keep the
/// **full blockstate-carrying** form — the model because waterlogging, slab
/// halves and snow layers change the fluid and step models, the
/// write-log because it is the runtime `setblock`/`fill` line. An air write
/// removes the cell (absent = air); any write replaces the whole block, state
/// included, so the cell's open-gate marking is re-derived from what was just
/// written rather than carried over.
///
/// **`open=true` is honoured** (island round 21). `Assembled::open_gates` is the
/// side set [`crate::assembled::occupancy_of`] reads to tell a *closed* fence
/// gate (a barrier the player opens with a right-click, and nobody else passes)
/// from an *open* one (a bare threshold). It was populated only by the prefab
/// read, and every edit write unconditionally CLEARED it — so a stage-7 edit
/// could write `minecraft:oak_fence_gate[open=true]`, ship that exact block in
/// the world, and still have every proof downstream model it as shut. That is
/// the model contradicting the bytes it emitted, and it made the one available
/// fix for `DW0452` impossible to author.
fn write_cell(
    assembled: &mut Assembled,
    batch_writes: &mut BTreeMap<[i32; 3], String>,
    cell: [i32; 3],
    block: &str,
) {
    if assembled::is_air(block) {
        assembled.blocks.remove(&cell);
    } else {
        assembled.blocks.insert(cell, block.to_string());
    }
    if assembled::is_fence_gate(block) && assembled::state_value(block, "open") == Some("true") {
        assembled.open_gates.insert(cell);
    } else {
        assembled.open_gates.remove(&cell);
    }
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
                        .is_some_and(|name| matches.contains(strip_ns(base_id(name))))
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
    bid: &str,
) -> Result<(), EditError> {
    if let MorphOp::Raise { recipe, .. } | MorphOp::Smooth { recipe, .. } = op {
        check_recipe(bid, "morph", recipe)?;
    }
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
            // Only a RAISING morph reaches this arm, and every raising morph
            // (`raise`/`smooth`) carries a recipe the caller already proved
            // non-empty — but never panic on content: a missing recipe skips
            // the raise rather than aborting the process.
            let Some((recipe, seed)) = recipe else { return };
            for y in (surf + 1)..=target {
                let cell = [x, y, z];
                let Some(block) = pick(recipe, noise_at(seed, cell, recipe)) else {
                    return;
                };
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
    Ok(())
}

/// The union `(x, z)` footprint of the keep-clear (`avoid`) regions.
fn avoid_footprint(
    bid: &str,
    regions: &BTreeMap<String, BTreeSet<[i32; 3]>>,
    avoid: &[delvewright_dsl::RegionId],
) -> Result<BTreeSet<(i32, i32)>, EditError> {
    let mut fp = BTreeSet::new();
    for r in avoid {
        let cells = regions.get(r.as_str()).ok_or_else(|| EditError {
            code: DW_EDIT_UNRESOLVED,
            message: format!(
                "world-edits batch `{bid}` avoids region `{r}` before any `select` defines it \
                 (validation should have caught this)"
            ),
        })?;
        fp.extend(cells.iter().map(|c| (c[0], c[2])));
    }
    Ok(fp)
}

/// Whether a region cell is a **standable dressing candidate**: air with an
/// occupied cell directly below (the generators' `is_solid(below) && is_air`
/// planting rule).
fn is_dressing_candidate(assembled: &Assembled, c: [i32; 3]) -> bool {
    !assembled.blocks.contains_key(&c) && assembled.blocks.contains_key(&[c[0], c[1] - 1, c[2]])
}

/// The `scatter` verb (spec-0017): seeded dressing over a region's
/// standable cells, honoring keep-clear envelopes. Ported from the greenfield
/// meadow's flower/dressing pass (`prefabs/island-terrain-generator`): raw
/// white-noise gates (dressing wants speckle, not the fill verbs' clustered
/// patches), weighted item pick per cell, optional both-axes spacing rule and
/// count cap taken in descending noise order — the generators' spread idiom.
#[allow(clippy::too_many_arguments)]
fn scatter(
    assembled: &mut Assembled,
    batch_writes: &mut BTreeMap<[i32; 3], String>,
    cells: &BTreeSet<[i32; 3]>,
    avoid_fp: &BTreeSet<(i32, i32)>,
    items: &[delvewright_dsl::PaletteBlock],
    density: f64,
    spacing: Option<u32>,
    limit: Option<u32>,
    seed: u64,
    bid: &str,
) -> Result<(), EditError> {
    // Defense in depth (map-editor audit): `dsl::validate` rejects an empty
    // `items` list, but `emit::build` is callable without it — a library caller
    // gets a structured error, never an index panic.
    let Some(last) = items.last() else {
        return Err(EditError {
            code: DW_EDIT_UNRESOLVED,
            message: format!(
                "world-edits batch `{bid}`: a `scatter` verb declares an empty `items` list, so \
                 it can place nothing (validation should have caught this — this build path \
                 skipped `dsl::validate`). Give the scatter at least one weighted item, or \
                 drop the edit"
            ),
        });
    };
    // Candidates: standable region cells off the keep-clear footprint, gated
    // by the density noise, carrying their placement-order noise.
    let mut cand: Vec<(f64, [i32; 3])> = Vec::new();
    for &c in cells {
        if avoid_fp.contains(&(c[0], c[2])) || !is_dressing_candidate(assembled, c) {
            continue;
        }
        if hash01(seed, c[0], c[1], c[2], 151) < density {
            cand.push((hash01(seed, c[0], c[1], c[2], 157), c));
        }
    }
    // Descending noise with fully deterministic tie-breaking (the generators'
    // sort), so `spacing`/`limit` keep the strongest candidates.
    cand.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let total: f64 = items.iter().map(|b| b.weight).sum();
    let cap = limit.map_or(usize::MAX, |l| l as usize);
    let sp = spacing.unwrap_or(0) as i32;
    let mut placed: Vec<(i32, i32)> = Vec::new();
    for (_, c) in cand {
        if placed.len() >= cap {
            break;
        }
        if sp > 0
            && placed
                .iter()
                .any(|&(px, pz)| (px - c[0]).abs() < sp && (pz - c[2]).abs() < sp)
        {
            continue; // reject only when close on BOTH axes (spread rule)
        }
        // Weighted item pick from an independent white-noise sample.
        let mut t = hash01(seed, c[0], c[1], c[2], 155).min(0.999_999) * total;
        let mut chosen = &last.block;
        for b in items {
            if t < b.weight {
                chosen = &b.block;
                break;
            }
            t -= b.weight;
        }
        write_cell(assembled, batch_writes, c, chosen);
        placed.push((c[0], c[2]));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The lean-or-grow canopy rules, ported from
// `prefabs/island-terrain-generator` — spec-0017's `plant` verb. The corridor
// rule is STRUCTURAL, never a cut: a canopy that would reach a keep-clear
// column first leans one block away; failing that the tree grows tall enough
// to arch its whole canopy over the corridor. No leaf is ever sliced.
// ---------------------------------------------------------------------------

/// Horizontal radius of the widest canopy layer and the squared radius that
/// rounds the leaf ball off (`dx² + dz² <= CANOPY_R2`). Generator constants.
const CANOPY_RAD: i32 = 2;
const CANOPY_R2: i32 = 5;
/// Full-headroom clearance a grown canopy leaves over a keep-clear column
/// (the generator's `G_CANOPY_CLEARANCE`).
const CANOPY_CLEARANCE: i32 = 3;

/// Whether a canopy blob centred at `(cx, cz)` would cover any keep-clear cell.
fn canopy_over_corridor(cx: i32, cz: i32, on_walk: &impl Fn(i32, i32) -> bool) -> bool {
    (-CANOPY_RAD..=CANOPY_RAD).any(|dx| {
        (-CANOPY_RAD..=CANOPY_RAD)
            .any(|dz| dx * dx + dz * dz <= CANOPY_R2 && on_walk(cx + dx, cz + dz))
    })
}

/// The one-block step that leans a canopy directly away from the nearest
/// keep-clear cell within `reach` of a trunk at `(x, z)`; `(0, 0)` when none
/// is in reach. Deterministic: nearest by squared distance, ties broken by
/// the fixed scan order.
fn corridor_lean(x: i32, z: i32, reach: i32, on_walk: &impl Fn(i32, i32) -> bool) -> (i32, i32) {
    let mut best: Option<(i32, i32, i32)> = None;
    for dx in -reach..=reach {
        for dz in -reach..=reach {
            if (dx, dz) == (0, 0) || !on_walk(x + dx, z + dz) {
                continue;
            }
            let d2 = dx * dx + dz * dz;
            if best.is_none_or(|(b, _, _)| d2 < b) {
                best = Some((d2, dx, dz));
            }
        }
    }
    best.map_or((0, 0), |(_, dx, dz)| (-dx.signum(), -dz.signum()))
}

/// Place one hand-shaped oak with its trunk feet at `(x, ty, z)` (`ty` = the
/// standable cell the trunk grows from). Verbatim port of the generator's
/// `place_oak` with the meadow's fixed walk plane generalized to the trunk's
/// own floor level; leaves only ever write into air, exactly as the generator
/// only wrote into in-bounds air.
fn place_oak(
    assembled: &mut Assembled,
    batch_writes: &mut BTreeMap<[i32; 3], String>,
    x: i32,
    ty: i32,
    z: i32,
    seed: u64,
    on_walk: &impl Fn(i32, i32) -> bool,
) {
    // 1. Lean away from the corridor; keep the small-oak height when that clears it.
    let (lean_x, lean_z) = corridor_lean(x, z, CANOPY_RAD + 1, on_walk);
    // 2. Otherwise centre the blob back on the trunk and lift it clear of the corridor.
    let (cx, cz, base) = if canopy_over_corridor(x + lean_x, z + lean_z, on_walk) {
        (x, z, ty + CANOPY_CLEARANCE)
    } else {
        let natural = ty + 1 + (value_noise(seed, x, 0, z, 0.6, 43) > 0.5) as i32;
        (x + lean_x, z + lean_z, natural)
    };
    let h = base + 1; // trunk top sits inside the blob's mid layer
    for y in ty..=h {
        write_cell(
            assembled,
            batch_writes,
            [x, y, z],
            "minecraft:oak_log[axis=y]",
        );
    }
    // Compact leaf ball: a rounded 5-wide band at the trunk top, narrowing upward.
    let leaf =
        |assembled: &mut Assembled, batch_writes: &mut BTreeMap<[i32; 3], String>, c: [i32; 3]| {
            if !assembled.blocks.contains_key(&c) {
                write_cell(
                    assembled,
                    batch_writes,
                    c,
                    "minecraft:oak_leaves[persistent=true]",
                );
            }
        };
    for dy in base..=(base + 2) {
        let rad = if dy == base + 2 { 1 } else { CANOPY_RAD };
        for dx in -rad..=rad {
            for dz in -rad..=rad {
                let r2 = dx * dx + dz * dz + (dy - h) * (dy - h);
                if r2 <= CANOPY_R2 {
                    leaf(assembled, batch_writes, [cx + dx, dy, cz + dz]);
                }
            }
        }
    }
    // A single crown leaf caps the ball.
    leaf(assembled, batch_writes, [cx, base + 3, cz]);
}

/// The `plant` verb (spec-0017): choose up to `count` trunk cells from
/// the region's highest-noise standable candidates (the generators' oak
/// selection: noise-descending with a both-axes spacing rule, salt 41), then
/// place each tree via the lean-or-grow canopy rules.
#[allow(clippy::too_many_arguments)]
fn plant(
    assembled: &mut Assembled,
    batch_writes: &mut BTreeMap<[i32; 3], String>,
    cells: &BTreeSet<[i32; 3]>,
    avoid_fp: &BTreeSet<(i32, i32)>,
    tree: delvewright_dsl::TreeKind,
    count: u32,
    spacing: u32,
    seed: u64,
) {
    let delvewright_dsl::TreeKind::Oak = tree;
    let on_walk = |x: i32, z: i32| avoid_fp.contains(&(x, z));
    let mut cand: Vec<(f64, [i32; 3])> = Vec::new();
    for &c in cells {
        if on_walk(c[0], c[2]) || !is_dressing_candidate(assembled, c) {
            continue;
        }
        cand.push((value_noise(seed, c[0], 5, c[2], 0.5, 41), c));
    }
    cand.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let sp = spacing as i32;
    let mut planted: Vec<(i32, i32)> = Vec::new();
    for (_, c) in cand {
        if planted.len() >= count as usize {
            break;
        }
        if planted
            .iter()
            .any(|&(px, pz)| (px - c[0]).abs() < sp && (pz - c[2]).abs() < sp)
        {
            continue;
        }
        place_oak(assembled, batch_writes, c[0], c[1], c[2], seed, &on_walk);
        planted.push((c[0], c[2]));
    }
}

/// The `fragment` verb (spec-0017): stamp a library prefab's non-air
/// cells at a frame-resolved position, optionally quarter-turned. The fragment
/// is a first-class library prefab — provenance/license live in its metadata
/// exactly like every placed prefab (ADR-0013), so nothing outside the
/// admitted library can be stamped. Semantically a `/place template` whose
/// bytes the compiler already models: non-air cells overwrite, authored air
/// does not erase (`structure_cells` yields non-air only).
#[allow(clippy::too_many_arguments)]
fn fragment(
    assembled: &mut Assembled,
    batch_writes: &mut BTreeMap<[i32; 3], String>,
    plan: &Plan,
    area: &crate::plan::AreaPlacement,
    bid: &str,
    prefabs: &crate::registry::PrefabRegistry,
    structures: &BTreeMap<String, Vec<u8>>,
    prefab: &delvewright_dsl::PrefabId,
    frame: &delvewright_dsl::EditFrame,
    at: [i32; 3],
    rotation: delvewright_dsl::FragmentRotation,
) -> Result<(), EditError> {
    let meta = prefabs.get(prefab.as_str()).ok_or_else(|| EditError {
        code: DW_EDIT_UNRESOLVED,
        message: format!(
            "world-edits batch `{bid}`: fragment prefab `{prefab}` is not in the prefab \
             library — only admitted library prefabs (with provenance/license metadata, \
             ADR-0013) can be stamped. Admit the fragment via `delve-admit` first, or fix \
             the id"
        ),
    })?;
    let origin = resolve_frame_point(plan, area, bid, frame, at).map_err(|message| EditError {
        code: DW_EDIT_UNRESOLVED,
        message,
    })?;
    let rot = match rotation {
        delvewright_dsl::FragmentRotation::None => crate::solver::Rotation::None,
        delvewright_dsl::FragmentRotation::Clockwise90 => crate::solver::Rotation::Cw90,
        delvewright_dsl::FragmentRotation::Clockwise180 => crate::solver::Rotation::Cw180,
        delvewright_dsl::FragmentRotation::Counterclockwise90 => crate::solver::Rotation::Ccw90,
    };
    // Blockstate-preserving read (map-editor audit, surfaced by DW0354): the
    // occupancy model stores bare ids, but a `fragment` stamp's writes ARE the
    // runtime `setblock` lines. Reading bare ids turned the hello-room prefab's
    // `lantern[hanging=true]` into a floor lantern stamped into mid-air, which
    // vanilla drops on the next chunk tick.
    //
    // Every template of the piece, at its piece-local offset: a stamped
    // fragment is the whole piece, and a piece past the vanilla cap ships as
    // several templates. Reading only the first would stamp part of a building
    // and report success — the failure a tile set has no other detector for.
    let mut cells: Vec<([i32; 3], String, Option<bool>)> = Vec::new();
    for template in meta.templates() {
        let bytes = structures.get(template.file).ok_or_else(|| EditError {
            code: DW_EDIT_UNRESOLVED,
            message: format!(
                "world-edits batch `{bid}`: fragment prefab `{prefab}`'s structure file \
                     `{}` was not loaded — the prefab library entry points at a missing `.nbt`",
                template.file
            ),
        })?;
        for (local, name, open) in assembled::structure_cells_stateful(bytes) {
            cells.push((
                [
                    local[0] + template.offset[0],
                    local[1] + template.offset[1],
                    local[2] + template.offset[2],
                ],
                name,
                open,
            ));
        }
    }
    if cells.is_empty() {
        return Err(EditError {
            code: DW_EDIT_UNRESOLVED,
            message: format!(
                "world-edits batch `{bid}`: fragment prefab `{prefab}` decodes to zero \
                 non-air cells — an empty stamp is a silent no-op; fix the `.nbt` or drop \
                 the edit"
            ),
        });
    }
    // `rotation` turns POSITIONS only — the compiler has no rotate-aware
    // blockstate rewriter, so a stamped `facing`/`axis`/`shape` would keep its
    // unrotated value and ship visibly deformed geometry (stairs facing into
    // walls, logs lying across the grain). That is the silently-deformed-map
    // class, so the compiler REFUSES rather than warns: a rotated stamp of a
    // prefab carrying any yaw-dependent property is a build error. Prefabs whose
    // states are provably yaw-INVARIANT (`hanging`, `half`, `waterlogged`,
    // `axis=y`, `facing=up|down`, …) rotate correctly and stay allowed — this is
    // a collision test, not a blanket ban on `rotation`.
    if rotation != delvewright_dsl::FragmentRotation::None
        && let Some((local, name, prop)) = first_yaw_dependent(&cells)
    {
        return Err(EditError {
            code: DW_EDIT_UNRESOLVED,
            message: format!(
                "world-edits batch `{bid}`: fragment prefab `{prefab}` is stamped with \
                 `rotation: {rotation:?}`, but it contains direction-bearing blockstates \
                 (`{name}` at prefab-local [{}, {}, {}] carries `{prop}`) and rotate-aware \
                 stamping is NOT implemented — the stamp turns cell POSITIONS only, so every \
                 `facing`/`axis`/`shape`/connection property would keep its unrotated value \
                 and the stamped geometry would ship visibly deformed. Stamp it unrotated, or \
                 admit a pre-rotated prefab variant to the library; do NOT stamp it rotated \
                 and hand-fix the facings downstream with extra edits",
                local[0], local[1], local[2],
            ),
        });
    }
    for (local, name, _open) in cells {
        let t = rot.transform(local);
        let cell = [origin[0] + t[0], origin[1] + t[1], origin[2] + t[2]];
        write_cell(assembled, batch_writes, cell, &name);
    }
    Ok(())
}

/// Blockstate properties whose value encodes a **horizontal** direction, and is
/// therefore wrong after a quarter-turn unless it is rewritten with the stamp:
///
/// * `facing` — stairs, doors, furnaces, wall torches, chests, … (values `up`
///   and `down` are yaw-invariant and excluded below)
/// * `axis` — logs/pillars/chains, where `x` and `z` swap (`y` is invariant)
/// * `shape` — stair corners (`inner_left`/`outer_right`/…) and rail bends
/// * `rotation` — the 16-step yaw of signs, banners and skulls
/// * `orientation` — jigsaw markers and crafters (`north_up`, …)
/// * `hinge` — a door's `left`/`right`, meaningful only against its `facing`
/// * `north`/`south`/`east`/`west` — the connection flags of fences, walls,
///   panes, redstone and mushroom blocks
///
/// Everything else a prefab can carry (`hanging`, `half`, `waterlogged`, `open`,
/// `lit`, `type`, `level`, `persistent`, `thickness`, `vertical_direction`, …)
/// is yaw-invariant, so a prefab built only from those rotates correctly.
const YAW_DEPENDENT_PROPS: &[&str] = &[
    "facing",
    "axis",
    "shape",
    "rotation",
    "orientation",
    "hinge",
    "north",
    "south",
    "east",
    "west",
];

/// The first `(local pos, block, "key=value")` in a decoded stamp whose state
/// depends on yaw, scanning in the decoder's deterministic cell order. `None`
/// when every state in the stamp is rotation-invariant.
fn first_yaw_dependent(
    cells: &[([i32; 3], String, Option<bool>)],
) -> Option<([i32; 3], &str, String)> {
    for (local, name, _) in cells {
        let Some(open) = name.find('[') else { continue };
        for pair in name[open + 1..name.len().saturating_sub(1)].split(',') {
            let Some((k, v)) = pair.split_once('=') else {
                continue;
            };
            // `axis=y` and `facing=up|down` name the vertical axis, which a
            // quarter-turn about y leaves untouched — never reject those.
            let invariant =
                (k == "axis" && v == "y") || (k == "facing" && (v == "up" || v == "down"));
            if YAW_DEPENDENT_PROPS.contains(&k) && !invariant {
                return Some((*local, base_id(name), pair.to_string()));
            }
        }
    }
    None
}

/// The `relight` verb (spec-0017): run the spec-0010 fixture-placement
/// pass over ONE region and bake the resulting fixtures into the edit script's
/// writes — authorial control of where fixtures land. Uses the exact
/// whole-area machinery (`light::relight_area`) constrained to the region's
/// AABB, with the area's declared `lighting` as the default spec and the
/// verb's `fixture`/`min_light` as overrides. A region the pass cannot light
/// is the same `DW0211` the area pass raises, batch-attributed.
#[allow(clippy::too_many_arguments)]
fn relight_region(
    assembled: &mut Assembled,
    batch_writes: &mut BTreeMap<[i32; 3], String>,
    fixture_cells: &mut BTreeSet<[i32; 3]>,
    plan: &Plan,
    batch: &delvewright_dsl::EditBatch,
    bid: &str,
    cells: &BTreeSet<[i32; 3]>,
    fixture: Option<delvewright_dsl::Fixture>,
    min_light: Option<u8>,
) -> Result<(), EditError> {
    let c = plan.campaign;
    let declared = c
        .world
        .content
        .areas
        .iter()
        .find(|a| a.id.as_str() == batch.area.as_str())
        .and_then(|a| a.lighting);
    // Defense in depth (map-editor audit): `dsl::validate` requires a
    // `fixture`/`min_light` override when the area declares no `lighting`
    // (DW0162), but `emit::build` is callable without validation — a library
    // caller gets a structured error, never a panic.
    let missing = |field: &str| EditError {
        code: DW_EDIT_UNRESOLVED,
        message: format!(
            "world-edits batch `{bid}`: a `relight` verb resolves no `{field}` — area `{}` \
             declares no `lighting` and the verb overrides none (validation should have caught \
             this — this build path skipped `dsl::validate`). Declare the area's `lighting`, or \
             give the verb an explicit `fixture` + `min_light`",
            batch.area
        ),
    };
    let spec = delvewright_dsl::AreaLighting {
        fixture: fixture
            .or(declared.map(|l| l.fixture))
            .ok_or_else(|| missing("fixture"))?,
        min_light: min_light
            .or(declared.map(|l| l.min_light))
            .ok_or_else(|| missing("min_light"))?,
    };
    let sky = crate::light::darkest_effective_sky(c);
    let nav = crate::nav::World::from_occupancy(assembled::occupancy_of(
        assembled.blocks.clone(),
        &assembled.open_gates,
    ));
    let moves = crate::nav::plan_moves(plan, &nav).unwrap_or_default();
    let required = nav.required_path_cells(plan, &moves);
    let (amin, amax) = match bounds_of(cells.iter()) {
        Some(b) => b,
        None => return Ok(()), // unreachable: used_region rejects empty regions
    };
    let starts = anchor_starts(plan);
    let reachable: BTreeSet<[i32; 3]> = nav
        .reachable_walkable_rooted(&starts)
        .into_iter()
        .filter(|cell| crate::light::in_bounds(*cell, amin, amax))
        .collect();
    if reachable.is_empty() {
        return Err(EditError {
            code: DW_EDIT_UNRESOLVED,
            message: format!(
                "world-edits batch `{bid}`: relight region contains no reachable walkable \
                 cell — there is nothing to light. Select a region covering floor the \
                 player can reach (check with `delvec blocking-chart`), or drop the edit"
            ),
        });
    }
    let mut model = crate::light::LightModel::from_blocks(assembled.blocks.clone());
    let mut out = crate::light::Relight::default();
    crate::light::relight_area(
        &mut model,
        &nav,
        &reachable,
        &required,
        &area_label(bid, batch),
        spec,
        sky,
        amin,
        amax,
        &mut out,
    );
    if let Some(diag) = out.diagnostics.first() {
        return Err(EditError {
            code: diag.code,
            message: format!("world-edits batch `{bid}`: {}", diag.message),
        });
    }
    for p in &out.placements {
        write_cell(assembled, batch_writes, p.pos, &p.block);
        // A `relight` placement is a DECLARED lighting guarantee: DW0354 treats
        // losing its support as an error, not an advisory.
        fixture_cells.insert(p.pos);
    }
    Ok(())
}

/// The area label a `relight` verb's diagnostics carry (`<area> (batch ...)`).
fn area_label(bid: &str, batch: &delvewright_dsl::EditBatch) -> String {
    format!("{} (batch `{bid}`)", batch.area)
}

/// Resolve a frame + offset to a world position (shared by box selects and
/// fragment stamps).
fn resolve_frame_point(
    plan: &Plan,
    area: &crate::plan::AreaPlacement,
    bid: &str,
    frame: &EditFrame,
    p: [i32; 3],
) -> Result<[i32; 3], String> {
    match frame {
        EditFrame::PieceLocal { piece, prefab } => {
            let idx = *piece as usize;
            let placed = area.pieces.get(idx).ok_or_else(|| {
                format!(
                    "world-edits batch `{bid}`: piece index {idx} is out of range — area `{}` \
                     placed {} piece(s) (0..={}). The layout this frame was authored against \
                     has drifted; re-inspect it with `delvec snapshot` and update the frame, \
                     do NOT guess an index",
                    area.area_id,
                    area.pieces.len(),
                    area.pieces.len().saturating_sub(1),
                )
            })?;
            if placed.prefab_id != prefab.as_str() {
                return Err(format!(
                    "world-edits batch `{bid}`: piece {idx} of area `{}` is `{}`, not the \
                     frame's declared `{prefab}` — the solved layout has drifted since this \
                     edit was authored. Re-inspect the layout (`delvec snapshot`) and \
                     re-target the frame; do NOT just delete the prefab guard",
                    area.area_id, placed.prefab_id,
                ));
            }
            let t = placed.rotation.transform(p);
            Ok([
                placed.pos[0] + t[0],
                placed.pos[1] + t[1],
                placed.pos[2] + t[2],
            ])
        }
        EditFrame::AnchorRelative { anchor } => {
            let key = (area.area_id.clone(), anchor.as_str().to_string());
            let pos = match plan.anchors.get(&key) {
                Some(ResolvedAnchor::Point { pos, .. }) => *pos,
                Some(ResolvedAnchor::Gate { from, .. }) => *from,
                None => {
                    return Err(format!(
                        "world-edits batch `{bid}`: area `{}` resolves no anchor `{anchor}` — \
                         use an anchor the area's placed prefab metadata declares (see the \
                         `delvec snapshot` manifest's target list)",
                        area.area_id,
                    ));
                }
            };
            Ok([pos[0] + p[0], pos[1] + p[1], pos[2] + p[2]])
        }
    }
}

/// Structure files referenced by `fragment` verbs but placed by no piece — the
/// extra `.nbt`s the CLI must load into the `structures` map before replay.
/// Unresolvable prefab ids are skipped here (the replay reports them as
/// `DW0323` with a remediation message).
pub fn fragment_structure_files(
    campaign: &delvewright_dsl::Campaign,
    prefabs: &crate::registry::PrefabRegistry,
) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    if let Some(env) = &campaign.world_edits {
        for batch in &env.content.batches {
            for edit in &batch.edits {
                if let WorldEdit::Fragment { prefab, .. } = edit
                    && let Some(meta) = prefabs.get(prefab.as_str())
                {
                    files.extend(meta.templates().iter().map(|t| t.file.to_string()));
                }
            }
        }
    }
    files
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
/// `None` for an empty palette (structurally rejected upstream by
/// [`check_recipe`]; returning an `Option` rather than indexing keeps a library
/// caller that skipped `dsl::validate` from panicking).
fn pick(recipe: &PaletteRecipe, n: f64) -> Option<&str> {
    let total: f64 = recipe.blocks.iter().map(|b| b.weight).sum();
    let mut t = n.min(0.999_999) * total;
    for b in &recipe.blocks {
        if t < b.weight {
            return Some(&b.block);
        }
        t -= b.weight;
    }
    recipe.blocks.last().map(|b| b.block.as_str())
}

/// The structured error an empty palette recipe raises. `dsl::validate` rejects
/// one (`DW0100`), but `emit::build` is callable without validation — defense in
/// depth against the map-editor audit's content-reachable panics.
fn empty_recipe(bid: &str, verb: &str) -> EditError {
    EditError {
        code: DW_EDIT_UNRESOLVED,
        message: format!(
            "world-edits batch `{bid}`: a `{verb}` verb declares an empty palette `recipe`, so \
             it can place nothing (validation should have caught this — this build path skipped \
             `dsl::validate`). Give the recipe at least one weighted block, or drop the edit"
        ),
    }
}

/// Reject an empty palette recipe before any cell is written.
fn check_recipe(bid: &str, verb: &str, recipe: &PaletteRecipe) -> Result<(), EditError> {
    if recipe.blocks.is_empty() {
        return Err(empty_recipe(bid, verb));
    }
    Ok(())
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
        assert_eq!(pick(&r, 0.0), Some("minecraft:stone"));
        assert_eq!(pick(&r, 0.74), Some("minecraft:stone"));
        assert_eq!(pick(&r, 0.76), Some("minecraft:mossy_cobblestone"));
        assert_eq!(pick(&r, 1.0), Some("minecraft:mossy_cobblestone"));
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

    /// A synthetic assembled world: a flat `stone` slab over `[0,w) × [0,d)`
    /// at `y = 0` (feet plane `y = 1`), nothing else.
    fn slab(w: i32, d: i32) -> Assembled {
        let mut blocks = BTreeMap::new();
        for x in 0..w {
            for z in 0..d {
                blocks.insert([x, 0, z], "minecraft:stone".to_string());
            }
        }
        Assembled {
            blocks,
            settled: Vec::new(),
            open_gates: BTreeSet::new(),
            gate_seals: Vec::new(),
        }
    }

    /// The air band over a slab (the standable dressing candidates).
    fn air_band(w: i32, d: i32) -> BTreeSet<[i32; 3]> {
        let mut cells = BTreeSet::new();
        for x in 0..w {
            for z in 0..d {
                cells.insert([x, 1, z]);
            }
        }
        cells
    }

    #[test]
    fn scatter_honors_avoid_and_is_deterministic() {
        let mut a = slab(8, 8);
        let mut writes = BTreeMap::new();
        let cells = air_band(8, 8);
        // Keep-clear: every column with x < 4.
        let avoid: BTreeSet<(i32, i32)> =
            (0..4).flat_map(|x| (0..8).map(move |z| (x, z))).collect();
        let items = vec![delvewright_dsl::PaletteBlock {
            block: "minecraft:poppy".to_string(),
            weight: 1.0,
        }];
        scatter(
            &mut a,
            &mut writes,
            &cells,
            &avoid,
            &items,
            1.0,
            None,
            None,
            42,
            "batch/test",
        )
        .unwrap();
        assert!(!writes.is_empty(), "density 1.0 dresses every free cell");
        assert!(
            writes.keys().all(|c| c[0] >= 4),
            "no dressing on keep-clear columns: {writes:?}"
        );
        // Deterministic: replaying over a fresh slab is identical.
        let mut b = slab(8, 8);
        let mut writes2 = BTreeMap::new();
        scatter(
            &mut b,
            &mut writes2,
            &cells,
            &avoid,
            &items,
            1.0,
            None,
            None,
            42,
            "batch/test",
        )
        .unwrap();
        assert_eq!(writes, writes2);
        // A different seed decorrelates the density gate at density < 1.
        let mut c1 = slab(8, 8);
        let mut w1 = BTreeMap::new();
        scatter(
            &mut c1,
            &mut w1,
            &cells,
            &avoid,
            &items,
            0.3,
            None,
            None,
            1,
            "batch/test",
        )
        .unwrap();
        let mut c2 = slab(8, 8);
        let mut w2 = BTreeMap::new();
        scatter(
            &mut c2,
            &mut w2,
            &cells,
            &avoid,
            &items,
            0.3,
            None,
            None,
            2,
            "batch/test",
        )
        .unwrap();
        assert_ne!(w1, w2, "seed decorrelates placement");
    }

    #[test]
    fn scatter_spacing_and_limit_bound_the_placements() {
        let mut a = slab(10, 10);
        let mut writes = BTreeMap::new();
        let cells = air_band(10, 10);
        let items = vec![delvewright_dsl::PaletteBlock {
            block: "minecraft:cobblestone".to_string(),
            weight: 1.0,
        }];
        scatter(
            &mut a,
            &mut writes,
            &cells,
            &BTreeSet::new(),
            &items,
            1.0,
            Some(4),
            Some(3),
            7,
            "batch/test",
        )
        .unwrap();
        assert!(writes.len() <= 3, "limit caps placements: {}", writes.len());
        let placed: Vec<[i32; 3]> = writes.keys().copied().collect();
        for (i, a1) in placed.iter().enumerate() {
            for b1 in &placed[i + 1..] {
                assert!(
                    (a1[0] - b1[0]).abs() >= 4 || (a1[2] - b1[2]).abs() >= 4,
                    "spacing rule: {a1:?} vs {b1:?}"
                );
            }
        }
    }

    #[test]
    fn corridor_lean_points_away_from_the_nearest_walk_cell() {
        let walk = |x: i32, z: i32| x == 2 && z == 0; // corridor east of the trunk
        assert_eq!(corridor_lean(0, 0, 3, &walk), (-1, 0), "leans west, away");
        let none = |_: i32, _: i32| false;
        assert_eq!(
            corridor_lean(0, 0, 3, &none),
            (0, 0),
            "no corridor, no lean"
        );
    }

    /// The structural rule: a canopy that cannot lean clear of a
    /// keep-clear column grows tall instead — nothing the tree writes sits in
    /// a keep-clear column below full clearance (trunk floor + 3), and no
    /// leaf is ever sliced to fake it.
    #[test]
    fn plant_grows_over_a_keep_clear_column_it_cannot_lean_off() {
        let mut a = slab(9, 9);
        let mut writes = BTreeMap::new();
        let cells = air_band(9, 9);
        // A keep-clear cross through the middle: leaning cannot clear it.
        let avoid: BTreeSet<(i32, i32)> = (0..9).flat_map(|i| [(i, 4), (4, i)]).collect();
        plant(
            &mut a,
            &mut writes,
            &cells,
            &avoid,
            delvewright_dsl::TreeKind::Oak,
            1,
            4,
            11,
        );
        assert!(
            writes.values().any(|b| b.starts_with("minecraft:oak_log")),
            "a tree was planted"
        );
        for c in writes.keys() {
            if avoid.contains(&(c[0], c[2])) {
                assert!(
                    c[1] > CANOPY_CLEARANCE,
                    "keep-clear column cell {c:?} below full clearance"
                );
            }
        }
    }

    #[test]
    fn plant_trunks_never_stand_on_keep_clear_columns() {
        let mut a = slab(9, 9);
        let mut writes = BTreeMap::new();
        let cells = air_band(9, 9);
        let avoid: BTreeSet<(i32, i32)> = (0..9).map(|z| (4, z)).collect();
        plant(
            &mut a,
            &mut writes,
            &cells,
            &avoid,
            delvewright_dsl::TreeKind::Oak,
            3,
            4,
            5,
        );
        for (c, b) in &writes {
            if b.starts_with("minecraft:oak_log") {
                assert!(!avoid.contains(&(c[0], c[2])), "trunk on keep-clear: {c:?}");
            }
        }
    }
}
