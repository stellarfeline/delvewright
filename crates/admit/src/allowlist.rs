//! The block-palette **allowlist** for the community-prefab NBT audit.
//!
//! POLICY (flagged for owner review — spec-0007 says "block-palette allowlist"
//! but does not enumerate it). The default is a deliberately broad set of vanilla
//! **building and decoration** blocks (the material a decorative dungeon prefab is
//! made of): stone/brick/wood/glass/copper/deepslate families plus common
//! decoration (stairs/slabs/walls/fences/doors/lights/carpets/banners), the inert
//! **flora** worldgen structures grow over themselves (grasses, flowers,
//! mushrooms, vines, coral, saplings), non-functional **furniture/job-site**
//! blocks (lectern, cartography table, loom, …), decorative **mineral** blocks and
//! ores, and archaeology (suspicious sand/gravel). It is intentionally *inclusive*
//! — the hard gate that actually protects the server is the code-injection forbid
//! list (command/structure blocks, NBT-bearing spawners), not this list. The
//! allowlist's job is to flag **surprising** blocks a reviewer should look at
//! (redstone contraptions — dispensers/droppers/pistons/observers/repeaters —,
//! tnt, note blocks/jukeboxes, deliberately left OUT), not to be an exhaustive
//! vanilla registry.
//!
//! The broadening (2026-07-31) was driven by the first real Modrinth ingestion
//! run: worldgen datapacks (ships, ruins, temples) are built from the full
//! vanilla flora + decoration vocabulary, and the narrower starter list rejected
//! every genuine hero piece on benign blocks (bare `glass_pane`, `short_grass`,
//! `emerald_block`, `cartography_table`, …) while its `forbidden` count stayed 0.
//! Still FLAGGED for owner ratification.
//!
//! The list is **configurable**: `Allowlist::from_file` loads a JSON
//! `{ "allow": ["minecraft:...", ...], "allow_suffixes": ["_stairs", ...] }` that
//! *replaces* the default, so the owner can tighten or widen it without a code
//! change. `jigsaw` is included by default because sockets are legitimate library
//! markers (carved during admission); it is NOT a code-injection vector (a jigsaw
//! block entity cannot carry a `Command`). This too is flagged for owner review.

use std::collections::BTreeSet;

use delvewright_schem::blocks::{BlockRegistry, LoadedId};
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

    /// **Judge a palette entry on the id the game will actually load.**
    ///
    /// This list is a list of names AT THE PIN. A template that pre-dates the
    /// pin writes its palette in an older vocabulary, and the game renames those
    /// ids on load — so testing the id AS WRITTEN asks a question about a
    /// spelling the running server never sees. That is not hypothetical: the
    /// audit's own spelling rule passes `minecraft:chain` in a DataVersion-2975
    /// template because the DataFixerUpper migrates it to `minecraft:iron_chain`
    /// (`DW0734`), and the next check refused the identical cell because
    /// `minecraft:chain` is not a name here (`DW0730`). One tool, two verdicts,
    /// on one block.
    ///
    /// The resolution is [`BlockRegistry::loaded_id_at`]'s, not this list's: what
    /// the pin holds is a fact about the game, and an allowlist that carried its
    /// own rename knowledge would be a second authority on it. What this method
    /// exists for is that the composition cannot be forgotten — there is no
    /// longer a way to ask this list about a palette entry without saying which
    /// `DataVersion` the entry was written at.
    ///
    /// An id the pin does not have and the rename table cannot reach is judged
    /// as written, which refuses it. That is the direction that stays sound: an
    /// id no fixer maps is a dead id, and a dead id in a palette is exactly what
    /// a reviewer must be shown.
    pub fn judge_entry<'a>(
        &self,
        entry: &'a PaletteEntry,
        registry: &'a BlockRegistry,
        data_version: i32,
    ) -> Judged<'a> {
        let (judged, renamed_from) = match registry.loaded_id_at(&entry.name, data_version) {
            LoadedId::Renamed { to, .. } => (to, Some(entry.name.as_str())),
            LoadedId::AsWritten | LoadedId::Unresolved => (entry.name.as_str(), None),
        };
        Judged {
            permitted: self.permits(judged),
            judged,
            renamed_from,
        }
    }
}

/// What [`Allowlist::judge_entry`] decided, and **which id it decided about**.
///
/// The second half is the point: a verdict on a pre-pin palette entry is a
/// verdict on a name the file does not contain, so a diagnostic that printed
/// only the written id would be describing a check it did not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Judged<'a> {
    /// Whether the allowlist permits it.
    pub permitted: bool,
    /// The id actually tested — what a 1.21.11 server holds after datafixing.
    pub judged: &'a str,
    /// The id as WRITTEN, when a rename was resolved on the way here.
    pub renamed_from: Option<&'a str>,
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
    // Decorative / flora families (all inert).
    "_bed",
    "_sapling",
    "_ore", // decorative/cave ore blocks (coal/iron/…/deepslate/nether variants)
    "_amethyst_bud",
    "_coral",
    "_coral_fan",
    "_coral_wall_fan",
    "_coral_block",
    "_mushroom_block",
    "_roots", // crimson/warped/hanging/mangrove roots
    "_skull", // decorative heads/skulls (inert as placed blocks)
    "_head",
    "_flower", // torchflower, wildflowers, cactus_flower, …
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
    "minecraft:iron_chain",
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
    // --- Bare-name building blocks the suffix families miss ---
    "minecraft:terracotta", // uncolored (the "_terracotta" suffix only catches dyed)
    "minecraft:glass_pane", // uncolored (the "_glass_pane" suffix only catches dyed)
    "minecraft:mud",
    "minecraft:mangrove_roots",
    "minecraft:muddy_mangrove_roots",
    "minecraft:mushroom_stem",
    "minecraft:sponge",
    "minecraft:wet_sponge",
    "minecraft:dried_kelp_block",
    "minecraft:nether_wart_block",
    "minecraft:warped_wart_block",
    "minecraft:crimson_nylium",
    "minecraft:warped_nylium",
    // --- Decorative mineral / gem blocks (treasure dressing) ---
    "minecraft:coal_block",
    "minecraft:diamond_block",
    "minecraft:emerald_block",
    "minecraft:lapis_block",
    "minecraft:netherite_block",
    "minecraft:raw_iron_block",
    "minecraft:raw_copper_block",
    "minecraft:raw_gold_block",
    // --- Archaeology ---
    "minecraft:suspicious_sand",
    "minecraft:suspicious_gravel",
    // --- Non-functional furniture / job-site blocks (village decoration) ---
    "minecraft:lectern",
    "minecraft:cartography_table",
    "minecraft:fletching_table",
    "minecraft:smithing_table",
    "minecraft:stonecutter",
    "minecraft:grindstone",
    "minecraft:loom",
    "minecraft:composter",
    "minecraft:beehive",
    "minecraft:bee_nest",
    "minecraft:amethyst_cluster",
    // --- Inert flora: grasses & foliage ---
    "minecraft:short_grass",
    "minecraft:tall_grass",
    "minecraft:fern",
    "minecraft:large_fern",
    "minecraft:bush",
    "minecraft:dead_bush",
    "minecraft:leaf_litter",
    "minecraft:lily_pad",
    "minecraft:pink_petals",
    "minecraft:wildflowers",
    "minecraft:spore_blossom",
    "minecraft:hanging_roots",
    "minecraft:big_dripleaf",
    "minecraft:big_dripleaf_stem",
    "minecraft:small_dripleaf",
    "minecraft:azalea",
    "minecraft:flowering_azalea",
    "minecraft:cactus",
    "minecraft:sugar_cane",
    "minecraft:bamboo",
    "minecraft:stripped_bamboo_block",
    "minecraft:sweet_berry_bush",
    "minecraft:cave_vines",
    "minecraft:cave_vines_plant",
    "minecraft:twisting_vines",
    "minecraft:twisting_vines_plant",
    "minecraft:weeping_vines",
    "minecraft:weeping_vines_plant",
    "minecraft:chorus_plant",
    "minecraft:chorus_flower",
    "minecraft:nether_wart",
    "minecraft:nether_sprouts",
    "minecraft:crimson_fungus",
    "minecraft:warped_fungus",
    "minecraft:brown_mushroom",
    "minecraft:red_mushroom",
    "minecraft:seagrass",
    "minecraft:tall_seagrass",
    "minecraft:kelp",
    "minecraft:kelp_plant",
    "minecraft:sea_pickle",
    // --- Inert flora: flowers ---
    "minecraft:dandelion",
    "minecraft:poppy",
    "minecraft:blue_orchid",
    "minecraft:allium",
    "minecraft:azure_bluet",
    "minecraft:red_tulip",
    "minecraft:orange_tulip",
    "minecraft:white_tulip",
    "minecraft:pink_tulip",
    "minecraft:oxeye_daisy",
    "minecraft:cornflower",
    "minecraft:lily_of_the_valley",
    "minecraft:wither_rose",
    "minecraft:sunflower",
    "minecraft:lilac",
    "minecraft:rose_bush",
    "minecraft:peony",
    "minecraft:pitcher_plant",
    "minecraft:open_eyeblossom",
    "minecraft:closed_eyeblossom",
];
