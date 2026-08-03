//! Cross-tileset generator invariants — one authority, five generators.
//!
//! The five tileset generators are deliberately separate Cargo workspaces
//! (`docs/reference/tools.md` §9) so that none of them can enter the shipped
//! `delvec`. That isolation is worth its cost, but it must not cost us the same
//! lesson five times: this file is included by every generator as
//! `#[path = "../../invariants.rs"] mod invariants;` — a source include, not a
//! dependency, so the workspaces stay independent while the rule stays single.
//!
//! Everything here is an `assert!`-style gate or the vocabulary a gate defines.
//! Running a generator is the test (`prefab-generators` CI job): it either emits
//! or panics.

#![allow(dead_code)]

use std::collections::BTreeMap;

/// A placed block, flattened out of a structure palette: name + block-state
/// properties. Each generator builds this view from the `Structure` it is about to
/// write, so the invariants see exactly the bytes that ship.
pub type Cells = BTreeMap<[i32; 3], (String, BTreeMap<String, String>)>;

/// Blocks that legitimately REST on a walkable tread, because vanilla defines them
/// as things that attach to a surface rather than as a mass of material: railings,
/// hardware, light fittings, soft cover. Nothing here reads as debris.
///
/// This list is a **decision point, not a bypass**. A block belongs here only if a
/// builder would deliberately put it on a step — and adding a full-cube material to
/// it would be weakening the check, which CLAUDE.md's debug doctrine forbids.
const TREAD_ATTACHMENTS: &[&str] = &[
    "_fence",
    "_fence_gate",
    "_pressure_plate",
    "_button",
    "_lever",
    "_torch",
    "_lantern",
    "_candle",
    "_sign",
    "_banner",
    "_carpet",
    "_rail",
    "_door",
    "_trapdoor",
    "_ladder",
    "_chain",
    "_sapling",
    "_bush",
    "_flower",
    "_mushroom",
    "_roots",
    "_sprouts",
    "_grass",
    "_fern",
    "_vine",
    "_pot",
    "_head",
    "_skull",
];

/// Exact names with no useful suffix, same rule as [`TREAD_ATTACHMENTS`].
const TREAD_ATTACHMENT_NAMES: &[&str] = &[
    "minecraft:torch",
    "minecraft:soul_torch",
    "minecraft:lantern",
    "minecraft:soul_lantern",
    "minecraft:chain",
    "minecraft:snow",
    "minecraft:vine",
    "minecraft:glow_lichen",
    "minecraft:cobweb",
    "minecraft:seagrass",
    "minecraft:tall_seagrass",
    "minecraft:kelp",
    "minecraft:kelp_plant",
    "minecraft:sea_pickle",
    "minecraft:lily_pad",
    "minecraft:bell",
    "minecraft:campfire",
    "minecraft:soul_campfire",
    "minecraft:end_rod",
    "minecraft:tripwire",
    "minecraft:tripwire_hook",
    "minecraft:water",
    "minecraft:lava",
];

fn is_air(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

fn may_rest_on_a_tread(name: &str) -> bool {
    TREAD_ATTACHMENT_NAMES.contains(&name) || TREAD_ATTACHMENTS.iter().any(|s| name.ends_with(s))
}

/// **Distress embeds, it never stacks.** (Owner playtest, island round 13: stray
/// stone sitting on the stair treads at the cave mouth — "凌乱感应该嵌入替换,而不是
/// 摞在上面".)
///
/// A stair is not decoration, it is the traversal surface itself: the thing a body
/// climbing the piece puts its feet on. A loose block resting on a tread is
/// therefore always wrong — it reads as litter dropped on the steps and it is what
/// the player collides with on the way up. Wear on a walked surface belongs *in*
/// the surface (a weathered variant of the same shape — see [`weathered`]), never
/// on top of it.
///
/// The gate: for every bottom-half stair with headroom (the cell two above it is
/// air, i.e. it is a tread a body can stand on and not a stair buried in a mass),
/// the cell **directly** above must be air or a [`TREAD_ATTACHMENTS`] fitting.
///
/// Deliberately scoped to stairs. A rubble mound on a flat floor is a legitimate,
/// multi-block dressing form the cave tileset builds on purpose; a lump on a tread
/// has no legitimate reading at all, so this is the boundary that can be stated
/// without a heuristic.
pub fn assert_distress_never_stacks(id: &str, cells: &Cells) {
    let name_at = |p: [i32; 3]| -> &str {
        cells
            .get(&p)
            .map(|(n, _)| n.as_str())
            .unwrap_or("minecraft:air")
    };
    let mut bad: Vec<([i32; 3], String, String)> = Vec::new();
    let mut treads = 0usize;
    for (pos, (name, props)) in cells {
        if !name.ends_with("_stairs") || props.get("half").map(String::as_str) == Some("top") {
            continue;
        }
        let up1 = [pos[0], pos[1] + 1, pos[2]];
        let up2 = [pos[0], pos[1] + 2, pos[2]];
        if !is_air(name_at(up2)) {
            continue; // buried in a mass: not a tread anyone walks
        }
        treads += 1;
        let on_it = name_at(up1);
        if is_air(on_it) || may_rest_on_a_tread(on_it) {
            continue;
        }
        bad.push((up1, on_it.to_string(), name.clone()));
    }
    assert!(
        bad.is_empty(),
        "{id}: {} of {treads} walkable stair tread(s) carry a stacked block — distress must be \
         EMBEDDED into the surface (weathered variant of the same stair, `invariants::weathered`), \
         never laid on top of it. Offenders: {}",
        bad.len(),
        bad.iter()
            .take(12)
            .map(|(p, on, tread)| format!("{on} at {p:?} on {tread}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
}

/// The weathered counterpart of a surface material — the vocabulary
/// [`assert_distress_never_stacks`] expects wear to be spoken in.
///
/// Shape is preserved by construction: a stair maps to a stair of the damaged
/// material, a slab to a slab, a full block to a full block, so the caller can keep
/// the original block state (facing / half / shape / waterlogged) verbatim and the
/// geometry a nav proof walked does not move. Every entry is a real 1.21.11 block —
/// there is, for instance, no `cracked_stone_brick_stairs` in vanilla, so cracked
/// stone brickwork weathers to its mossy stair rather than to an invented one.
///
/// `None` means "this surface has no weathered form": the caller then leaves the
/// surface untouched and drops the distress, which still satisfies the invariant.
pub fn weathered(name: &str) -> Option<&'static str> {
    Some(match name {
        "minecraft:stone" => "minecraft:cobblestone",
        "minecraft:stone_stairs" => "minecraft:cobblestone_stairs",
        "minecraft:stone_slab" => "minecraft:cobblestone_slab",
        "minecraft:cobblestone" => "minecraft:mossy_cobblestone",
        "minecraft:cobblestone_stairs" => "minecraft:mossy_cobblestone_stairs",
        "minecraft:cobblestone_slab" => "minecraft:mossy_cobblestone_slab",
        "minecraft:cobblestone_wall" => "minecraft:mossy_cobblestone_wall",
        "minecraft:stone_bricks" => "minecraft:cracked_stone_bricks",
        "minecraft:stone_brick_stairs" => "minecraft:mossy_stone_brick_stairs",
        "minecraft:stone_brick_slab" => "minecraft:mossy_stone_brick_slab",
        "minecraft:stone_brick_wall" => "minecraft:mossy_stone_brick_wall",
        "minecraft:polished_andesite" => "minecraft:andesite",
        "minecraft:polished_andesite_stairs" => "minecraft:andesite_stairs",
        "minecraft:polished_andesite_slab" => "minecraft:andesite_slab",
        "minecraft:polished_deepslate" => "minecraft:cobbled_deepslate",
        "minecraft:polished_deepslate_stairs" => "minecraft:cobbled_deepslate_stairs",
        "minecraft:polished_deepslate_slab" => "minecraft:cobbled_deepslate_slab",
        "minecraft:deepslate_bricks" => "minecraft:cracked_deepslate_bricks",
        "minecraft:deepslate_tiles" => "minecraft:cracked_deepslate_tiles",
        "minecraft:bricks" => "minecraft:mud_bricks",
        "minecraft:oak_planks" => "minecraft:stripped_oak_log",
        "minecraft:spruce_planks" => "minecraft:stripped_spruce_log",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(name: &str) -> (String, BTreeMap<String, String>) {
        (name.to_string(), BTreeMap::new())
    }

    fn tread(pos: [i32; 3], cells: &mut Cells) {
        let mut props = BTreeMap::new();
        props.insert("half".into(), "bottom".into());
        props.insert("facing".into(), "north".into());
        cells.insert(pos, ("minecraft:cobblestone_stairs".into(), props));
    }

    /// The island round-13 shape: a loose rock on a tread with headroom above it.
    #[test]
    #[should_panic(expected = "walkable stair tread(s) carry a stacked block")]
    fn a_rock_on_a_tread_fails() {
        let mut cells = Cells::new();
        tread([0, 0, 0], &mut cells);
        cells.insert([0, 1, 0], cell("minecraft:mossy_cobblestone"));
        assert_distress_never_stacks("fixture", &cells);
    }

    /// The same tread, weathered instead of littered — what the fix emits.
    #[test]
    fn a_weathered_tread_passes() {
        let mut cells = Cells::new();
        let mut props = BTreeMap::new();
        props.insert("half".into(), "bottom".into());
        props.insert("facing".into(), "north".into());
        cells.insert(
            [0, 0, 0],
            ("minecraft:mossy_cobblestone_stairs".into(), props),
        );
        assert_distress_never_stacks("fixture", &cells);
    }

    /// Hardware and railings are placed on steps on purpose (the galleon's rail,
    /// the cistern's pressure plates) — they are not debris.
    #[test]
    fn an_attachment_on_a_tread_passes() {
        for attachment in [
            "minecraft:oak_fence",
            "minecraft:oak_fence_gate",
            "minecraft:stone_pressure_plate",
            "minecraft:torch",
            "minecraft:short_grass",
        ] {
            let mut cells = Cells::new();
            tread([0, 0, 0], &mut cells);
            cells.insert([0, 1, 0], cell(attachment));
            assert_distress_never_stacks("fixture", &cells);
        }
    }

    /// A stair with no headroom is masonry inside a mass, not a surface anyone
    /// walks — the rule is about treads, so it must not fire there.
    #[test]
    fn a_stair_buried_in_a_mass_is_not_a_tread() {
        let mut cells = Cells::new();
        tread([0, 0, 0], &mut cells);
        cells.insert([0, 1, 0], cell("minecraft:cobblestone"));
        cells.insert([0, 2, 0], cell("minecraft:cobblestone"));
        assert_distress_never_stacks("fixture", &cells);
    }

    /// A top-half stair is a ceiling piece; nothing stands on it.
    #[test]
    fn a_ceiling_stair_is_not_a_tread() {
        let mut props = BTreeMap::new();
        props.insert("half".into(), "top".into());
        let mut cells = Cells::new();
        cells.insert([0, 0, 0], ("minecraft:cobblestone_stairs".into(), props));
        cells.insert([0, 1, 0], cell("minecraft:mossy_cobblestone"));
        assert_distress_never_stacks("fixture", &cells);
    }

    /// Wear keeps the shape it wore: a stair weathers to a stair, a slab to a slab.
    /// A mapping that changed shape would move geometry a nav proof already walked.
    #[test]
    fn weathering_preserves_the_shape_family() {
        for (from, to) in [
            (
                "minecraft:cobblestone_stairs",
                "minecraft:mossy_cobblestone_stairs",
            ),
            (
                "minecraft:stone_brick_stairs",
                "minecraft:mossy_stone_brick_stairs",
            ),
            (
                "minecraft:cobblestone_slab",
                "minecraft:mossy_cobblestone_slab",
            ),
            ("minecraft:stone_bricks", "minecraft:cracked_stone_bricks"),
        ] {
            assert_eq!(weathered(from), Some(to));
            let shape = |n: &str| {
                if n.ends_with("_stairs") {
                    "stairs"
                } else if n.ends_with("_slab") {
                    "slab"
                } else {
                    "block"
                }
            };
            assert_eq!(
                shape(from),
                shape(to),
                "{from} changed shape when weathered"
            );
        }
        assert_eq!(weathered("minecraft:gravel"), None);
    }
}
