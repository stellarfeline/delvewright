//! The shared assembled-world block model (task #42).
//!
//! One authoritative cell→block map of the world the shipped delve actually
//! assembles, built the way vanilla does: place each prefab structure with
//! `/place template <pos> <rotation>`, apply the solver's socket seals (air fill
//! opens a mated socket; wall material seals an unused one), clear gate
//! thresholds — **and then settle gravity-affected blocks**, because the delve
//! ships into a `the_void` flat world (validation/compose.yaml) with no natural
//! floor, so an unsupported `sand`/`gravel`/… block placed by `/place template`
//! immediately falls out of the world and leaves air.
//!
//! Both downstream models derive from this one map (CLAUDE.md "no hacks at any
//! layer"; the fix belongs in the shared model, not at each consumer):
//!
//! - [`crate::nav::World`] — collision/standability for `move-npc`, cutscene,
//!   the DW0311 critical-path walk, DW0312 wave seating, and the waypoint export.
//! - [`crate::light::LightModel`] — opacity/emission for the spec-0010 relight.
//!
//! ## Why settling is required for fidelity (task #42)
//!
//! Field bug: the cave-den prefab floor is a single layer of blocks over void.
//! Where that layer is `minecraft:sand` (a [`FallingBlock`], gravity-affected),
//! the block is placed by `/place template` and then falls into the void on the
//! next block update — the assembled game world has a *hole* there. The old
//! occupancy model counted every non-air block as permanent solid floor (it had
//! no gravity model), so it "proved" a solid floor the game does not have and
//! seated wave mobs / routed the player over void. Settling here makes the model
//! match the game: an unsupported falling block is removed (it despawned into the
//! void); a falling block with solid support somewhere below rests on top of it,
//! exactly as the entity would land.
//!
//! Determinism (ADR-0006): placement/seal/gate order is fixed, and settling
//! iterates `BTreeMap`-ordered columns and stacks with a fixed tie-break — same
//! DSL + seed → identical map.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

use crate::plan::{Plan, ResolvedAnchor};

/// The air block variants that count as "no block" (passable / transparent).
pub fn is_air(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

/// Whether a block falls under gravity when the cell below cannot support it
/// (vanilla `FallingBlock`). In the delve's `the_void` world such a block, placed
/// unsupported by `/place template`, drops out of the world and leaves air — so
/// the assembled-world model must not treat it as permanent floor.
///
/// Deliberately excludes `pointed_dripstone` and `scaffolding`: those attach
/// upward / by support-distance rather than resting on the block directly below,
/// so the "supported from below" settling rule here does not model them — a cave
/// generator hangs dripstone *stalactites* from the ceiling, and this rule must
/// never mistake a ceiling-hung block for an unsupported one and delete it.
pub fn is_falling_block(name: &str) -> bool {
    let id = name.strip_prefix("minecraft:").unwrap_or(name);
    // NB: `suspicious_sand`/`suspicious_gravel` are brushable archaeology blocks,
    // NOT `FallingBlock` — they stay put, so they are deliberately excluded.
    matches!(
        id,
        "sand" | "red_sand" | "gravel" | "anvil" | "chipped_anvil" | "damaged_anvil" | "dragon_egg"
    ) || id.ends_with("_concrete_powder")
}

/// Inclusive cell iterator over a region given two (possibly unordered) corners.
pub fn region_cells(a: [i32; 3], b: [i32; 3]) -> impl Iterator<Item = [i32; 3]> {
    let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    (lo[1]..=hi[1]).flat_map(move |y| {
        (lo[2]..=hi[2]).flat_map(move |z| (lo[0]..=hi[0]).map(move |x| [x, y, z]))
    })
}

/// Parse a gzipped vanilla structure `.nbt`, returning its non-air block cells as
/// `(local [x, y, z], block id)`. Unparseable structures contribute nothing.
pub fn structure_named_cells(bytes: &[u8]) -> Vec<([i32; 3], String)> {
    let mut raw = Vec::new();
    if flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut raw)
        .is_err()
    {
        return Vec::new();
    }
    let Ok(fastnbt::Value::Compound(root)) = fastnbt::from_bytes::<fastnbt::Value>(&raw) else {
        return Vec::new();
    };
    let palette: Vec<Option<String>> = match root.get("palette") {
        Some(fastnbt::Value::List(entries)) => entries
            .iter()
            .map(|e| match e {
                fastnbt::Value::Compound(c) => match c.get("Name") {
                    Some(fastnbt::Value::String(s)) => Some(s.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect(),
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    if let Some(fastnbt::Value::List(blocks)) = root.get("blocks") {
        for b in blocks {
            let fastnbt::Value::Compound(b) = b else {
                continue;
            };
            let pos = match b.get("pos") {
                Some(fastnbt::Value::List(p)) if p.len() == 3 => {
                    let mut o = [0i32; 3];
                    let mut ok = true;
                    for (i, v) in p.iter().enumerate() {
                        match v {
                            fastnbt::Value::Int(n) => o[i] = *n,
                            _ => ok = false,
                        }
                    }
                    if !ok {
                        continue;
                    }
                    o
                }
                _ => continue,
            };
            let state = match b.get("state") {
                Some(fastnbt::Value::Int(n)) => *n as usize,
                _ => continue,
            };
            if let Some(Some(name)) = palette.get(state)
                && !is_air(name)
            {
                out.push((pos, name.clone()));
            }
        }
    }
    out
}

/// The un-settled cell→block map: placed structures + solver seals + gate clears,
/// exactly as the two legacy models built it. Kept separate from settling so unit
/// tests can exercise each half.
fn placed_blocks(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
) -> BTreeMap<[i32; 3], String> {
    let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let Some(bytes) = structures.get(&piece.structure_file) else {
                continue;
            };
            for (local, name) in structure_named_cells(bytes) {
                let t = piece.rotation.transform(local);
                blocks.insert(
                    [
                        piece.pos[0] + t[0],
                        piece.pos[1] + t[1],
                        piece.pos[2] + t[2],
                    ],
                    name,
                );
            }
        }
        // Seals land after placement: an air fill opens a mated socket; anything
        // else seals an unused one.
        for s in &area.seals {
            if is_air(&s.block) {
                for cell in region_cells(s.from, s.to) {
                    blocks.remove(&cell);
                }
            } else {
                for cell in region_cells(s.from, s.to) {
                    blocks.insert(cell, s.block.clone());
                }
            }
        }
    }
    // Gate thresholds are passable (an open-gate effect fills them with air).
    for resolved in plan.anchors.values() {
        if let ResolvedAnchor::Gate { from, to, .. } = resolved {
            for cell in region_cells(*from, *to) {
                blocks.remove(&cell);
            }
        }
    }
    blocks
}

/// The outcome of gravity settling for one falling block: the world cell it ended
/// up at (or `None` if it despawned into the void), tagged with its original cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settled {
    /// The falling block's id (e.g. `minecraft:sand`).
    pub block: String,
    /// The block's authored (placed) world cell before settling.
    pub from: [i32; 3],
    /// Where it came to rest, or `None` if it fell out of the world (despawned).
    pub to: Option<[i32; 3]>,
}

/// Settle every gravity-affected block in `blocks` as vanilla physics would in the
/// void world: within each `(x, z)` column, non-falling blocks are immovable
/// supports; each falling block drops onto the highest support at or below it
/// (stacking in original order), and a falling block with no support anywhere
/// below it despawns (falls out of the world → air). Mutates `blocks` in place and
/// returns one [`Settled`] per falling block (its authored cell and where it
/// ended up), so callers can distinguish a benign land-on-support from a despawn.
///
/// Deterministic (ADR-0006): columns iterate in `BTreeMap` order and blocks stack
/// bottom-up.
fn settle(blocks: &mut BTreeMap<[i32; 3], String>) -> Vec<Settled> {
    // Group cell y's by column.
    let mut columns: BTreeMap<(i32, i32), Vec<i32>> = BTreeMap::new();
    for c in blocks.keys() {
        columns.entry((c[0], c[2])).or_default().push(c[1]);
    }
    let mut outcomes: Vec<Settled> = Vec::new();
    for ((x, z), mut ys) in columns {
        ys.sort_unstable();
        // Split the column into immovable supports and falling blocks (ascending).
        let mut fixed: Vec<i32> = Vec::new();
        let mut falling: Vec<(i32, String)> = Vec::new();
        for y in &ys {
            let name = &blocks[&[x, *y, z]];
            if is_falling_block(name) {
                falling.push((*y, name.clone()));
            } else {
                fixed.push(*y);
            }
        }
        if falling.is_empty() {
            continue; // nothing to settle in this column
        }
        // Lift every falling block out; re-drop onto its support.
        for (y, _) in &falling {
            blocks.remove(&[x, *y, z]);
        }
        // Group falling blocks by the nearest immovable support strictly below
        // them; a group has no support → those blocks despawned into the void.
        let mut by_base: BTreeMap<i32, Vec<(i32, String)>> = BTreeMap::new();
        for (y, name) in falling {
            if let Some(base) = fixed.iter().copied().filter(|&f| f < y).max() {
                by_base.entry(base).or_default().push((y, name));
            } else {
                // No support anywhere below → despawns into the void.
                outcomes.push(Settled {
                    block: name,
                    from: [x, y, z],
                    to: None,
                });
            }
        }
        for (base, group) in by_base {
            // Stack from base+1 upward; the group came from the open gap above
            // `base`, so it cannot overrun the next support — but skip any fixed
            // cell defensively.
            let mut yy = base + 1;
            for (from_y, name) in group {
                while fixed.binary_search(&yy).is_ok() {
                    yy += 1;
                }
                outcomes.push(Settled {
                    block: name.clone(),
                    from: [x, from_y, z],
                    to: Some([x, yy, z]),
                });
                blocks.insert([x, yy, z], name);
                yy += 1;
            }
        }
    }
    outcomes
}

/// The authoritative assembled-world model: the settled cell→block map plus the
/// per-falling-block settle outcomes (for the gravity-despawn diagnostic).
pub struct Assembled {
    /// The gravity-settled cell→block map (cells absent from it are air).
    pub blocks: BTreeMap<[i32; 3], String>,
    /// One outcome per falling block: where it came to rest, or `None` if it
    /// despawned into the void.
    pub settled: Vec<Settled>,
}

/// Assemble the world: placed structures + solver seals + gate clears, then
/// gravity-settle, returning both the settled map and the per-falling-block
/// outcomes. Shared root for [`assembled_blocks`] and the gravity-despawn check.
pub fn assemble(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Assembled {
    let mut blocks = placed_blocks(plan, structures);
    let settled = settle(&mut blocks);
    Assembled { blocks, settled }
}

/// The authoritative assembled-world cell→block map: placed structures + solver
/// seals + gate clears, **then gravity-settled**. Cells absent from the map are
/// air. Shared by the nav occupancy model and the relight light model so a single
/// gravity-faithful world feeds every consumer (task #42).
pub fn assembled_blocks(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
) -> BTreeMap<[i32; 3], String> {
    assemble(plan, structures).blocks
}

/// `DW0313`: one or more placed gravity blocks despawn into the void at placement.
/// A gravity floor (`sand`/`gravel`/…) laid unsupported over the delve's `the_void`
/// world falls out of the world on the first block update, silently deforming the
/// shipped map (holes, light leaks, visual damage) even where no critical path or
/// wave seat happens to cross it — so DW0311/DW0312 alone would let it ship green.
/// This is the authoritative, direct gate: no DSL verb can intend a despawn, so it
/// is always a prefab/generator defect (task #42, owner addendum).
pub const DW_GRAVITY_DESPAWN: &str = "DW0313";

/// A placed piece's prefab id paired with its world AABB `(min, max)`, for
/// attributing a despawned cell back to the piece that placed it.
type PieceBox<'a> = (&'a str, ([i32; 3], [i32; 3]));

/// Build the [`DW_GRAVITY_DESPAWN`] (`DW0313`) message if any placed gravity block
/// despawns into the void, attributing the despawned cells to the prefab piece
/// that placed them; `None` when the assembled world settles with zero losses.
///
/// A gravity block that merely *falls onto support* is NOT flagged here: the
/// settled model represents it faithfully, so every consumer (nav, light,
/// waypoints) already ships the true geometry — there is no correctness gap, and
/// the generator's own zero-unsupported invariant catches an *unintended* fall at
/// authoring time. Only a despawn — an irrecoverable hole — is a build error.
pub fn gravity_despawn_error(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
) -> Option<String> {
    let assembled = assemble(plan, structures);
    // (prefab_id, world AABB) for every placed piece, for despawn attribution.
    let pieces: Vec<PieceBox> = plan
        .areas
        .iter()
        .flat_map(|a| a.pieces.iter())
        .map(|p| (p.prefab_id.as_str(), p.bbox()))
        .collect();
    despawn_message(&assembled.settled, &pieces)
}

/// Pure core of [`gravity_despawn_error`]: the `DW0313` message for the despawned
/// members of `settled`, attributing each despawned cell to the prefab piece whose
/// AABB contains it; `None` when nothing despawns. Split out so the diagnostic's
/// attribution and #73-rubric wording are unit-testable without a full [`Plan`].
fn despawn_message(settled: &[Settled], pieces: &[PieceBox]) -> Option<String> {
    let despawned: Vec<&Settled> = settled.iter().filter(|s| s.to.is_none()).collect();
    if despawned.is_empty() {
        return None;
    }

    // Pieces never overlap (the solver rejects overlaps), so the first containing
    // AABB is the unique placer.
    let piece_of = |cell: [i32; 3]| -> &str {
        pieces
            .iter()
            .find(|(_, (lo, hi))| (0..3).all(|i| lo[i] <= cell[i] && cell[i] <= hi[i]))
            .map(|(id, _)| *id)
            .unwrap_or("<unknown piece>")
    };

    // Group despawned cells + block kinds per piece (deterministic BTreeMap).
    type PerPiece<'a> = BTreeMap<&'a str, (Vec<[i32; 3]>, BTreeSet<String>)>;
    let mut per_piece: PerPiece = BTreeMap::new();
    for s in &despawned {
        let entry = per_piece.entry(piece_of(s.from)).or_default();
        entry.0.push(s.from);
        entry.1.insert(s.block.replace("minecraft:", ""));
    }

    let total = despawned.len();
    let mut summaries = Vec::new();
    for (piece, (mut cells, kinds)) in per_piece {
        cells.sort_unstable();
        let sample: Vec<String> = cells.iter().take(4).map(|c| format!("{c:?}")).collect();
        let more = cells.len().saturating_sub(sample.len());
        let kinds: Vec<&str> = kinds.iter().map(String::as_str).collect();
        summaries.push(format!(
            "`{piece}` — {n} cell(s) of {kinds} at {sample}{extra}",
            n = cells.len(),
            kinds = kinds.join("/"),
            sample = sample.join(", "),
            extra = if more > 0 {
                format!(" (+{more} more)")
            } else {
                String::new()
            },
        ));
    }

    Some(format!(
        "gravity settling: {total} placed gravity block(s) fall out of the world at placement and \
         despawn into the void, leaving holes in the assembled floor. The delve ships into a \
         `the_void` world, so a gravity block ({kinds_hint}) with no solid block directly beneath it \
         is unsupported and drops away on the first block update. Affected: {summary}. \
         WHERE to fix: the prefab / tileset generator that produced these piece(s), not the compiler. \
         HOW: give every gravity floor cell a non-falling SUPPORT block directly beneath it — a \
         substrate layer under the visible surface, exactly as `cave-shore`'s beach seabed is built \
         (every sand/gravel cell rests on solid rock). Do NOT swap the floor palette to non-falling \
         blocks (sandstone/tuff/stone) just to silence this: sand/gravel floors are a supported, \
         first-class content need — the fix is to add the substrate, never to remove the material.",
        kinds_hint = "sand/red_sand/gravel/concrete_powder/anvil/dragon_egg",
        summary = summaries.join("; "),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_falling_block_over_void_despawns() {
        // A single layer of sand at y=64 over void — the cave-den floor field bug:
        // every unsupported sand cell falls out of the world.
        let mut blocks = BTreeMap::new();
        blocks.insert([0, 64, 0], "minecraft:sand".to_string());
        blocks.insert([1, 64, 0], "minecraft:sand".to_string());
        settle(&mut blocks);
        assert!(
            blocks.is_empty(),
            "unsupported sand must despawn: {blocks:?}"
        );
    }

    #[test]
    fn falling_block_rests_on_support_and_fills_a_gap() {
        // stone at y=64 (support), air at 65, sand at 66 → sand falls to 65.
        let mut blocks = BTreeMap::new();
        blocks.insert([0, 64, 0], "minecraft:stone".to_string());
        blocks.insert([0, 66, 0], "minecraft:sand".to_string());
        settle(&mut blocks);
        assert_eq!(
            blocks.get(&[0, 64, 0]).map(String::as_str),
            Some("minecraft:stone")
        );
        assert_eq!(
            blocks.get(&[0, 65, 0]).map(String::as_str),
            Some("minecraft:sand")
        );
        assert!(
            !blocks.contains_key(&[0, 66, 0]),
            "sand should have fallen off 66"
        );
    }

    #[test]
    fn stacked_falling_blocks_settle_contiguously_on_support() {
        // stone(64), sand(66), sand(68) → both sand fall onto the stone: 65,66.
        let mut blocks = BTreeMap::new();
        blocks.insert([0, 64, 0], "minecraft:stone".to_string());
        blocks.insert([0, 66, 0], "minecraft:sand".to_string());
        blocks.insert([0, 68, 0], "minecraft:gravel".to_string());
        settle(&mut blocks);
        assert!(blocks.contains_key(&[0, 64, 0]));
        assert!(blocks.contains_key(&[0, 65, 0]));
        assert!(blocks.contains_key(&[0, 66, 0]));
        assert!(!blocks.contains_key(&[0, 67, 0]));
        assert!(!blocks.contains_key(&[0, 68, 0]));
    }

    #[test]
    fn non_falling_blocks_float_and_are_untouched() {
        // Stone floats in vanilla; a floating andesite floor over void stays put —
        // this is why the cave-den andesite/coarse_dirt cells survived while the
        // sand cells fell.
        let mut blocks = BTreeMap::new();
        blocks.insert([0, 64, 0], "minecraft:andesite".to_string());
        blocks.insert([1, 64, 0], "minecraft:coarse_dirt".to_string());
        let before = blocks.clone();
        settle(&mut blocks);
        assert_eq!(blocks, before);
    }

    #[test]
    fn cave_den_field_bug_mixed_floor_keeps_only_the_non_falling_cells() {
        // The exact task #42 field shape: a single-layer cave floor over void,
        // part `sand` (falling) and part `andesite` (fixed). In game every sand
        // cell falls out of the void and leaves a hole; every andesite cell
        // stays. The model must reproduce this — seating a wave or routing a path
        // over the sand region has no footing.
        let mut blocks = BTreeMap::new();
        for x in 0..5 {
            let name = if x < 3 {
                "minecraft:sand"
            } else {
                "minecraft:andesite"
            };
            blocks.insert([x, 64, 0], name.to_string());
        }
        settle(&mut blocks);
        for x in 0..3 {
            assert!(
                !blocks.contains_key(&[x, 64, 0]),
                "sand floor cell {x} must fall into the void"
            );
        }
        for x in 3..5 {
            assert_eq!(
                blocks.get(&[x, 64, 0]).map(String::as_str),
                Some("minecraft:andesite"),
                "andesite floor cell {x} must remain"
            );
        }
    }

    #[test]
    fn dripstone_is_not_settled_as_a_falling_floor() {
        // pointed_dripstone hangs from the ceiling (attaches upward); the settle
        // rule must not treat it as an unsupported floor block and delete it.
        assert!(!is_falling_block("minecraft:pointed_dripstone"));
    }

    #[test]
    fn settle_reports_despawns_and_landings_distinctly() {
        // Column A (x=0): sand over void -> despawns (to == None).
        // Column B (x=1): stone support, air gap, sand above -> lands on support.
        let mut blocks = BTreeMap::new();
        blocks.insert([0, 64, 0], "minecraft:sand".to_string());
        blocks.insert([1, 64, 0], "minecraft:stone".to_string());
        blocks.insert([1, 66, 0], "minecraft:sand".to_string());
        let outcomes = settle(&mut blocks);
        let despawned: Vec<_> = outcomes.iter().filter(|s| s.to.is_none()).collect();
        let landed: Vec<_> = outcomes.iter().filter(|s| s.to.is_some()).collect();
        assert_eq!(despawned.len(), 1, "one sand cell despawns: {outcomes:?}");
        assert_eq!(despawned[0].from, [0, 64, 0]);
        assert_eq!(landed.len(), 1, "one sand cell lands on support");
        assert_eq!(landed[0].from, [1, 66, 0]);
        assert_eq!(landed[0].to, Some([1, 65, 0]));
    }

    #[test]
    fn despawn_message_fires_with_attribution_and_anti_dodge_wording() {
        // An unsupported sand floor in a synthetic piece "prefab/test-den" whose
        // AABB covers x0..4,y64,z0. Every sand cell despawns.
        let mut blocks = BTreeMap::new();
        for x in 0..5 {
            blocks.insert([x, 64, 0], "minecraft:sand".to_string());
        }
        let settled = settle(&mut blocks);
        let pieces = [("prefab/test-den", ([0, 64, 0], [4, 64, 0]))];
        let msg = despawn_message(&settled, &pieces).expect("despawn must be flagged");
        // WHAT: count + kind; WHERE: the piece; HOW + anti-dodge clause (#73 rubric).
        assert!(msg.contains("despawn"), "names the failure: {msg}");
        assert!(
            msg.contains("prefab/test-den"),
            "attributes the piece: {msg}"
        );
        assert!(msg.contains("sand"), "names the block kind: {msg}");
        assert!(msg.contains("substrate"), "prescribes the fix: {msg}");
        assert!(
            msg.contains("Do NOT swap the floor palette"),
            "carries the anti-dodge clause: {msg}"
        );
        // A supported floor produces no message.
        let mut ok = BTreeMap::new();
        for x in 0..5 {
            ok.insert([x, 63, 0], "minecraft:stone".to_string());
            ok.insert([x, 64, 0], "minecraft:sand".to_string());
        }
        let ok_settled = settle(&mut ok);
        assert!(
            despawn_message(&ok_settled, &pieces).is_none(),
            "supported floor is clean"
        );
    }

    #[test]
    fn substrate_under_gravity_floor_settles_with_zero_despawns() {
        // The generator's fix shape: a non-falling substrate (stone) beneath every
        // gravity surface cell -> nothing despawns, and the surface is preserved.
        let mut blocks = BTreeMap::new();
        for x in 0..5 {
            blocks.insert([x, 63, 0], "minecraft:stone".to_string()); // substrate
            blocks.insert([x, 64, 0], "minecraft:sand".to_string()); // surface
        }
        let outcomes = settle(&mut blocks);
        assert!(
            outcomes.iter().all(|s| s.to == Some(s.from)),
            "every supported gravity cell stays put: {outcomes:?}"
        );
        for x in 0..5 {
            assert_eq!(
                blocks.get(&[x, 64, 0]).map(String::as_str),
                Some("minecraft:sand")
            );
            assert_eq!(
                blocks.get(&[x, 63, 0]).map(String::as_str),
                Some("minecraft:stone")
            );
        }
    }
}
