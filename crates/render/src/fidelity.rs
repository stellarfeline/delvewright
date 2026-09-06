//! The fidelity-gate fixture: an in-code structure packed with the **newest
//! 1.21.11 blocks** (pale oak, crafter, copper family, tuff set, trial-chamber
//! set, …). `delvec render fidelity-gate` renders it and fails if any block meshes
//! as the magenta missing-texture placeholder (a texture-resolution regression).
//!
//! `minecraft:heavy_core` is deliberately **excluded**: its bare
//! `"texture":"all"` model is unresolved by Nucleation and always renders as a
//! placeholder (a known upstream gap, spike-render-fidelity). The gate must pass
//! on real content, so the expected-fail block is kept out — while the detector's
//! ability to catch a real placeholder is proven separately against a committed
//! `heavy_core` crop (`detect::tests::catches_real_heavy_core_placeholder`).

use crate::nbt::Structure;

/// The newest-block showcase, one block per Z, on a stone-brick floor so every
/// face is visible. `heavy_core` is intentionally absent (see module docs).
const SHOWCASE: &[&str] = &[
    "minecraft:pale_oak_planks",
    "minecraft:pale_oak_log[axis=y]",
    "minecraft:crafter[crafting=false,orientation=north_up,triggered=false]",
    "minecraft:copper_bulb[lit=true,powered=false]",
    "minecraft:copper_grate",
    "minecraft:waxed_copper_grate",
    "minecraft:tuff_bricks",
    "minecraft:chiseled_tuff",
    "minecraft:polished_tuff",
    "minecraft:tuff_brick_stairs[facing=east,half=bottom,shape=straight]",
    "minecraft:trial_spawner",
    "minecraft:vault",
    "minecraft:chiseled_copper",
    "minecraft:copper_door[facing=north,half=lower,hinge=left,open=false,powered=false]",
    "minecraft:stone_bricks",
    "minecraft:chiseled_stone_bricks",
    "minecraft:glowstone",
    "minecraft:iron_bars",
];

/// Build the fidelity-gate fixture structure directly as a [`Structure`] (the
/// same shape [`crate::nbt::parse_structure`] produces), so it flows through the
/// same `build_schematic` → render path as a real prefab.
pub fn fixture_structure() -> Structure {
    let depth = SHOWCASE.len() as i32;
    let mut palette: Vec<String> = vec!["minecraft:stone_bricks".to_string()];
    let floor_idx = 0usize;
    let mut blocks: Vec<([i32; 3], usize)> = Vec::new();

    for z in 0..depth {
        // Floor row (y=0) across x=0..3.
        for x in 0..3 {
            blocks.push(([x, 0, z], floor_idx));
        }
        // Showcase block at the centre column, y=1.
        let state = SHOWCASE[z as usize];
        let idx = match palette.iter().position(|s| s == state) {
            Some(i) => i,
            None => {
                palette.push(state.to_string());
                palette.len() - 1
            }
        };
        blocks.push(([1, 1, z], idx));
    }

    Structure {
        size: [3, 2, depth],
        palette,
        blocks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_excludes_heavy_core_and_covers_newest_blocks() {
        let st = fixture_structure();
        assert!(
            !st.palette.iter().any(|s| s.contains("heavy_core")),
            "heavy_core must stay OUT of the gate fixture (expected-fail)"
        );
        // Sanity: the newest-era blocks are present.
        for needle in [
            "pale_oak_planks",
            "crafter",
            "copper_bulb",
            "tuff_bricks",
            "chiseled_copper",
        ] {
            assert!(
                st.palette.iter().any(|s| s.contains(needle)),
                "fixture missing {needle}"
            );
        }
        assert_eq!(st.size, [3, 2, SHOWCASE.len() as i32]);
    }
}
