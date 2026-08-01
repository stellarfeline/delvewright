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

use std::collections::BTreeMap;
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

/// Settle every gravity-affected block in `blocks` as vanilla physics would in the
/// void world: within each `(x, z)` column, non-falling blocks are immovable
/// supports; each falling block drops onto the highest support at or below it
/// (stacking in original order), and a falling block with no support anywhere
/// below it despawns (falls out of the world → air). Mutates `blocks` in place.
///
/// Deterministic (ADR-0006): columns iterate in `BTreeMap` order and blocks stack
/// bottom-up.
fn settle(blocks: &mut BTreeMap<[i32; 3], String>) {
    // Group cell y's by column.
    let mut columns: BTreeMap<(i32, i32), Vec<i32>> = BTreeMap::new();
    for c in blocks.keys() {
        columns.entry((c[0], c[2])).or_default().push(c[1]);
    }
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
        let mut by_base: BTreeMap<i32, Vec<String>> = BTreeMap::new();
        for (y, name) in falling {
            if let Some(base) = fixed.iter().copied().filter(|&f| f < y).max() {
                by_base.entry(base).or_default().push(name);
            }
        }
        for (base, names) in by_base {
            // Stack from base+1 upward; the group came from the open gap above
            // `base`, so it cannot overrun the next support — but skip any fixed
            // cell defensively.
            let mut yy = base + 1;
            for name in names {
                while fixed.binary_search(&yy).is_ok() {
                    yy += 1;
                }
                blocks.insert([x, yy, z], name);
                yy += 1;
            }
        }
    }
}

/// The authoritative assembled-world cell→block map: placed structures + solver
/// seals + gate clears, **then gravity-settled**. Cells absent from the map are
/// air. Shared by the nav occupancy model and the relight light model so a single
/// gravity-faithful world feeds every consumer (task #42).
pub fn assembled_blocks(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
) -> BTreeMap<[i32; 3], String> {
    let mut blocks = placed_blocks(plan, structures);
    settle(&mut blocks);
    blocks
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
}
