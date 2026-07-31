//! The block-palette **allowlist** for the community-prefab NBT audit.
//!
//! POLICY (flagged for owner review — spec-0007 says "block-palette allowlist"
//! but does not enumerate it). The default is a deliberately broad set of vanilla
//! **building** blocks (the material a decorative dungeon prefab is made of):
//! stone/brick/wood/glass/copper/deepslate families plus common decoration
//! (stairs/slabs/walls/fences/doors/lights/carpets/banners). It is intentionally
//! *inclusive* — the hard gate that actually protects the server is the
//! code-injection forbid list (command/structure blocks, NBT-bearing spawners),
//! not this list. The allowlist's job is to flag **surprising** blocks a reviewer
//! should look at (e.g. redstone contraptions, tnt, mob heads), not to be an
//! exhaustive vanilla registry.
//!
//! The list is **configurable**: `Allowlist::from_file` loads a JSON
//! `{ "allow": ["minecraft:...", ...], "allow_suffixes": ["_stairs", ...] }` that
//! *replaces* the default, so the owner can tighten or widen it without a code
//! change. `jigsaw` is included by default because sockets are legitimate library
//! markers (carved during admission); it is NOT a code-injection vector (a jigsaw
//! block entity cannot carry a `Command`). This too is flagged for owner review.

use std::collections::BTreeSet;

use serde::Deserialize;

use crate::structure::PaletteEntry;

/// A resolved allowlist: exact ids plus name-suffix families.
#[derive(Debug, Clone)]
pub struct Allowlist {
    exact: BTreeSet<String>,
    suffixes: Vec<String>,
}

/// The on-disk override shape.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowlistFile {
    #[serde(default)]
    allow: Vec<String>,
    #[serde(default)]
    allow_suffixes: Vec<String>,
}

impl Allowlist {
    /// The built-in default (broad vanilla building set).
    pub fn default_building() -> Allowlist {
        let exact: BTreeSet<String> = DEFAULT_EXACT.iter().map(|s| s.to_string()).collect();
        let suffixes: Vec<String> = DEFAULT_SUFFIXES.iter().map(|s| s.to_string()).collect();
        Allowlist { exact, suffixes }
    }

    /// Load a JSON override that fully replaces the default.
    pub fn from_file(text: &str) -> Result<Allowlist, String> {
        let f: AllowlistFile =
            serde_json::from_str(text).map_err(|e| format!("invalid allowlist file: {e}"))?;
        Ok(Allowlist {
            exact: f.allow.into_iter().collect(),
            suffixes: f.allow_suffixes,
        })
    }

    /// True when a block name is permitted (exact id or a suffix family).
    pub fn permits(&self, name: &str) -> bool {
        if self.exact.contains(name) {
            return true;
        }
        let local = name.split_once(':').map(|(_, p)| p).unwrap_or(name);
        self.suffixes.iter().any(|suf| local.ends_with(suf))
    }

    /// True when a whole palette entry is permitted (checks the name only —
    /// properties do not affect admissibility).
    pub fn permits_entry(&self, entry: &PaletteEntry) -> bool {
        self.permits(&entry.name)
    }
}

impl Default for Allowlist {
    fn default() -> Self {
        Allowlist::default_building()
    }
}

/// Suffix families (any block whose local name ends with one of these). Covers the
/// combinatorial block sets so the exact list stays readable.
const DEFAULT_SUFFIXES: &[&str] = &[
    "_stairs",
    "_slab",
    "_wall",
    "_fence",
    "_fence_gate",
    "_door",
    "_trapdoor",
    "_planks",
    "_log",
    "_wood",
    "_leaves",
    "_bricks",
    "_brick",
    "_tiles",
    "_tile",
    "_pillar",
    "_carpet",
    "_wool",
    "_terracotta",
    "_concrete",
    "_concrete_powder",
    "_glass",
    "_glass_pane",
    "_banner",
    "_pressure_plate",
    "_button",
    "_sign",
    "_hanging_sign",
    "_lantern",
    "_torch",
    "_candle",
    "_stem",
    "_hyphae",
    "_shulker_box",
    "_glazed_terracotta",
    "_wall_hanging_sign",
    "_wall_sign",
];

/// Exact ids that are not covered by a suffix family.
const DEFAULT_EXACT: &[&str] = &[
    "minecraft:air",
    "minecraft:cave_air",
    "minecraft:void_air",
    "minecraft:stone",
    "minecraft:cobblestone",
    "minecraft:mossy_cobblestone",
    "minecraft:smooth_stone",
    "minecraft:andesite",
    "minecraft:diorite",
    "minecraft:granite",
    "minecraft:polished_andesite",
    "minecraft:polished_diorite",
    "minecraft:polished_granite",
    "minecraft:deepslate",
    "minecraft:cobbled_deepslate",
    "minecraft:polished_deepslate",
    "minecraft:chiseled_deepslate",
    "minecraft:tuff",
    "minecraft:polished_tuff",
    "minecraft:chiseled_tuff",
    "minecraft:calcite",
    "minecraft:dripstone_block",
    "minecraft:pointed_dripstone",
    "minecraft:chiseled_stone_bricks",
    "minecraft:cracked_stone_bricks",
    "minecraft:mossy_stone_bricks",
    "minecraft:chiseled_sandstone",
    "minecraft:cut_sandstone",
    "minecraft:smooth_sandstone",
    "minecraft:sandstone",
    "minecraft:red_sandstone",
    "minecraft:cut_red_sandstone",
    "minecraft:smooth_red_sandstone",
    "minecraft:chiseled_red_sandstone",
    "minecraft:bricks",
    "minecraft:mud_bricks",
    "minecraft:packed_mud",
    "minecraft:dirt",
    "minecraft:coarse_dirt",
    "minecraft:rooted_dirt",
    "minecraft:grass_block",
    "minecraft:podzol",
    "minecraft:gravel",
    "minecraft:sand",
    "minecraft:red_sand",
    "minecraft:clay",
    "minecraft:obsidian",
    "minecraft:crying_obsidian",
    "minecraft:netherrack",
    "minecraft:nether_bricks",
    "minecraft:red_nether_bricks",
    "minecraft:chiseled_nether_bricks",
    "minecraft:cracked_nether_bricks",
    "minecraft:blackstone",
    "minecraft:polished_blackstone",
    "minecraft:polished_blackstone_bricks",
    "minecraft:chiseled_polished_blackstone",
    "minecraft:gilded_blackstone",
    "minecraft:basalt",
    "minecraft:polished_basalt",
    "minecraft:smooth_basalt",
    "minecraft:end_stone",
    "minecraft:end_stone_bricks",
    "minecraft:purpur_block",
    "minecraft:purpur_pillar",
    "minecraft:quartz_block",
    "minecraft:chiseled_quartz_block",
    "minecraft:quartz_pillar",
    "minecraft:smooth_quartz",
    "minecraft:quartz_bricks",
    "minecraft:prismarine",
    "minecraft:prismarine_bricks",
    "minecraft:dark_prismarine",
    "minecraft:sea_lantern",
    "minecraft:glowstone",
    "minecraft:shroomlight",
    "minecraft:froglight",
    "minecraft:ochre_froglight",
    "minecraft:verdant_froglight",
    "minecraft:pearlescent_froglight",
    "minecraft:magma_block",
    "minecraft:redstone_lamp",
    "minecraft:copper_block",
    "minecraft:exposed_copper",
    "minecraft:weathered_copper",
    "minecraft:oxidized_copper",
    "minecraft:cut_copper",
    "minecraft:exposed_cut_copper",
    "minecraft:weathered_cut_copper",
    "minecraft:oxidized_cut_copper",
    "minecraft:chiseled_copper",
    "minecraft:copper_bulb",
    "minecraft:copper_grate",
    "minecraft:iron_block",
    "minecraft:iron_bars",
    "minecraft:chain",
    "minecraft:gold_block",
    "minecraft:bookshelf",
    "minecraft:chiseled_bookshelf",
    "minecraft:barrel",
    "minecraft:chest",
    "minecraft:crafting_table",
    "minecraft:furnace",
    "minecraft:smoker",
    "minecraft:cauldron",
    "minecraft:water_cauldron",
    "minecraft:flower_pot",
    "minecraft:lodestone",
    "minecraft:lightning_rod",
    "minecraft:decorated_pot",
    "minecraft:tinted_glass",
    "minecraft:glass",
    "minecraft:cobweb",
    "minecraft:hay_block",
    "minecraft:bell",
    "minecraft:end_rod",
    "minecraft:lantern",
    "minecraft:soul_lantern",
    "minecraft:torch",
    "minecraft:soul_torch",
    "minecraft:campfire",
    "minecraft:soul_campfire",
    "minecraft:jack_o_lantern",
    "minecraft:carved_pumpkin",
    "minecraft:pumpkin",
    "minecraft:melon",
    "minecraft:vine",
    "minecraft:glow_lichen",
    "minecraft:moss_block",
    "minecraft:moss_carpet",
    "minecraft:pale_moss_block",
    "minecraft:water",
    "minecraft:lava",
    "minecraft:ice",
    "minecraft:packed_ice",
    "minecraft:blue_ice",
    "minecraft:snow",
    "minecraft:snow_block",
    "minecraft:powder_snow",
    "minecraft:ladder",
    "minecraft:scaffolding",
    "minecraft:rail",
    "minecraft:soul_sand",
    "minecraft:soul_soil",
    "minecraft:bone_block",
    "minecraft:amethyst_block",
    "minecraft:budding_amethyst",
    "minecraft:jigsaw",
    "minecraft:structure_void",
    "minecraft:barrier",
    "minecraft:light",
];
