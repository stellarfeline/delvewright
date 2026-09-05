//! Cross-tileset generator invariants — one authority, every generator.
//!
//! The generators under `prefabs/*-generator` are deliberately separate Cargo
//! workspaces (`docs/reference/tools.md` §9) so that none of them can enter the
//! shipped `delvec`. That isolation is worth its cost, but it must not cost us
//! the same lesson once per generator: this file is included by every one as
//! `#[path = "../../invariants.rs"] mod invariants;` — a source include, not a
//! dependency, so the workspaces stay independent while the rule stays single.
//!
//! Everything here is an `assert!`-style gate or the vocabulary a gate defines.
//! Running a generator is the test (`prefab-generators` CI job): it either emits
//! or panics.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::OnceLock;

/// The pinned 1.21.11 block-state registry, source-included the same way this
/// file is: one authority, every generator, no dependency edge.
///
/// `crates/schem` parses the identical file for the in-workspace emitters
/// (`delvewright_schem::blocks`). Two readers of one file is not two authorities
/// — the alternative here would be a *further* hand-maintained block list, which
/// is the defect this gate exists to catch.
const BLOCK_REGISTRY_JSON: &str = include_str!("../crates/dsl/data/blocks-1.21.11.json");

/// **What a block state does to a body that walks into it** — the same module
/// `delvec`, the grammar back end and the admission pipeline all read
/// (`delvewright_dsl::blockshape`, spec-0056), source-included the same way the
/// registry above is.
///
/// It sits beside [`fluid`] under this name deliberately: `fluid` reaches it as
/// `super::blockshape`, which resolves inside `delvewright-schem` — where the
/// crate root re-exports it — and here, where this module is its neighbour. An
/// absolute `delvewright_dsl::` path would resolve in the workspace and not in a
/// generator, and the include would break the day the two touched.
#[path = "../crates/dsl/src/blockshape.rs"]
#[allow(dead_code)]
pub mod blockshape;

/// **What a cell does when there is fluid beside it** — the same module the
/// in-workspace auditor reads (`delvewright_schem::fluid`), source-included the
/// same way the registry above is.
///
/// Every fact in it was measured on the pinned server, and two of them are the
/// opposite of what a reader would guess (`waterlogged=true` does not spread;
/// `waterlogged=false` is a wall). Restating them here would be a second
/// authority on a question that already has one, and the two would agree right
/// up until they did not.
#[path = "../crates/schem/src/fluid.rs"]
#[allow(dead_code)]
pub mod fluid;

fn block_registry() -> &'static BTreeMap<String, BTreeMap<String, Vec<String>>> {
    static REGISTRY: OnceLock<BTreeMap<String, BTreeMap<String, Vec<String>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        serde_json::from_str(BLOCK_REGISTRY_JSON).expect("the vendored block registry parses")
    })
}

/// **A generator may only emit blocks the pinned game actually has.**
///
/// (One such block is `minecraft:chain`; the pinned game holds
/// `minecraft:iron_chain`.) A structure template carrying an unknown
/// block id loads it as **air**. So this defect costs the whole feature — eight
/// cells of bell-rope in `tk-bell-tower.nbt` — while the generator exits 0, the
/// `.nbt` round-trips, the byte-identity check passes, and nothing anywhere
/// says a word. It is the exact shape of the `delvec prefab` finding CLAUDE.md
/// records: *a command whose response nobody reads cannot fail*, one layer
/// down, on blocks instead of commands.
///
/// Property names and values are checked too, because vanilla drops the whole
/// state — not just the offending property — when it cannot parse one.
///
/// A non-`minecraft:` namespace is left alone: this registry has nothing to say
/// about a datapack's own blocks.
pub fn assert_blocks_are_real(id: &str, cells: &Cells) {
    let registry = block_registry();
    let mut bad: BTreeMap<String, usize> = BTreeMap::new();
    for (name, props) in cells.values() {
        if !name.starts_with("minecraft:") {
            continue;
        }
        let reason = match registry.get(name) {
            None => format!("{name} is not a block in Minecraft 1.21.11"),
            Some(known) => {
                let mut reason = None;
                for (property, value) in props {
                    match known.get(property) {
                        None => {
                            reason = Some(format!("{name} has no property {property:?}"));
                            break;
                        }
                        Some(legal) if !legal.contains(value) => {
                            reason =
                                Some(format!("{name}[{property}={value}] is not a legal state"));
                            break;
                        }
                        Some(_) => {}
                    }
                }
                match reason {
                    Some(r) => r,
                    None => continue,
                }
            }
        };
        *bad.entry(reason).or_insert(0) += 1;
    }
    assert!(
        bad.is_empty(),
        "{id}: the piece places block states Minecraft 1.21.11 does not have. A structure \
         template loads an unknown block as AIR, so this ships a hole and reports nothing. \
         Offenders: {}",
        bad.iter()
            .map(|(reason, cells)| format!("{reason} ({cells} cell(s))"))
            .collect::<Vec<_>>()
            .join("; ")
    );
}

/// A placed block, flattened out of a structure palette: name + block-state
/// properties. Each generator builds this view from the `Structure` it is about to
/// write, so the invariants see exactly the bytes that ship.
pub type Cells = BTreeMap<[i32; 3], (String, BTreeMap<String, String>)>;

/// What [`assert_fluid_is_contained`] examined: the numbers a caller prints so
/// the gate's binding is in the generator's own output rather than implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FluidBinding {
    /// Fluid **sources** examined — the gate's binding count. Zero means the
    /// piece has no body of fluid and the rule said nothing about it, which is
    /// a different fact from "the rule holds here".
    pub examined: usize,
    /// Cells written `waterlogged=true`: wet, measured not to spread, under no
    /// containment obligation.
    pub held: usize,
    /// Run directions that leave the piece's own outer face. Counted, never
    /// failed — what is beyond a face is not in these bytes.
    pub at_edge: usize,
}

/// **A body of fluid stays where the generator wrote it.**
///
/// Water and lava are the one thing an emitter places that does not stay
/// placed: they run down first, then sideways, into any open cell, on the
/// server's own clock and before any player arrives. So a generator that writes
/// a pool is asserting something about every cell around it — and no generator
/// checked that assertion until this gate existed. `cave-shore.nbt` shipped a
/// 33-cell sea written one block PROUD of the beach it laps, with seven ways
/// out into the air over the sand; the generator exited 0, the bytes
/// round-tripped, the determinism gate passed, and every tool in the repo drew
/// it as still water.
///
/// The block knowledge is not restated here — [`fluid`] is the same module
/// `delvec prefab audit` reads for `DW0800`, so the emitter and the auditor share
/// one rule instead of two that agree until they do not. What is local is the
/// walk over this generator's own cells, because the grid type is.
///
/// Two obligations, and they are one rule (*this fluid is where it was put and
/// stays there*):
///
/// - **saturated** — every fluid cell is a source. `level` is a value vanilla
///   derives from a cell's neighbours and re-derives on its own clock, so a
///   piece cannot pin one.
/// - **contained** — nothing open below or beside a source. Open means AIR, and
///   a cell the piece does not write is air: that is what `/place template`
///   leaves, which is why this needs the piece's `size` rather than only its
///   written cells.
///
/// Fluid never runs upward, so an authored pool's open top is not a leak.
///
/// A run that leaves the piece's own outer face is **counted and never failed**:
/// what is beyond that face is not in these bytes — a shoreline piece's water is
/// whatever it is placed against. That is also the direction in which this gate
/// could be answered rather than fixed, so the count is returned for the caller
/// to print on every run instead of living in this comment.
pub fn assert_fluid_is_contained(id: &str, size: [i32; 3], cells: &Cells) -> FluidBinding {
    let inside = |p: [i32; 3]| {
        (0..size[0]).contains(&p[0]) && (0..size[1]).contains(&p[1]) && (0..size[2]).contains(&p[2])
    };
    // Down first, then the four sides: the order the fluid itself takes, so the
    // first named leak is the one an author will see first.
    let runs: [[i32; 3]; 5] = [[0, -1, 0], [0, 0, -1], [0, 0, 1], [1, 0, 0], [-1, 0, 0]];
    let mut binding = FluidBinding::default();
    let mut bad: Vec<String> = Vec::new();

    for (pos, (name, props)) in cells {
        match fluid::wetness(name, props) {
            fluid::Wetness::Dry => continue,
            fluid::Wetness::Held => {
                binding.held += 1;
                continue;
            }
            fluid::Wetness::Flowing(level) => {
                binding.examined += 1;
                bad.push(format!(
                    "{pos:?} is {name} at level={level} — mid-flow, not a source"
                ));
                continue;
            }
            fluid::Wetness::Source => binding.examined += 1,
        }
        for step in runs {
            let into = [pos[0] + step[0], pos[1] + step[1], pos[2] + step[2]];
            if !inside(into) {
                binding.at_edge += 1;
                continue;
            }
            match cells.get(&into) {
                // Unwritten is air, and air is a way out.
                None => bad.push(format!("{pos:?} runs into {into:?} (unwritten, i.e. air)")),
                Some((n, _)) if fluid::is_structure_void(n) => binding.at_edge += 1,
                // Another fluid cell is the same body, not a way out of it.
                Some((n, _)) if fluid::is_fluid(n) => {}
                Some((n, p)) if !fluid::holds_fluid(n, p) => {
                    bad.push(format!("{pos:?} runs into {into:?} ({n})"))
                }
                Some(_) => {}
            }
        }
    }

    assert!(
        bad.is_empty(),
        "{id}: {} way(s) out of {} fluid source(s) ({} at the piece's own outer face, which this \
         gate counts and does not judge). A body of fluid is saturated and walled BY \
         CONSTRUCTION: every cell a source, and nothing open beside or below it. This piece \
         renders as still water in every tool here and runs on the first tick in the world. \
         Offenders: {}",
        bad.len(),
        binding.examined,
        binding.at_edge,
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("; ")
    );
    // The binding, on every run and per piece. A generator has no
    // machine-readable report to carry it, so its stdout is the artifact — and a
    // gate that examined nothing has to be able to say so out loud, or a piece
    // that quietly lost its water reads exactly like a piece that holds it.
    println!(
        "  fluid-contained {id}: {} source(s) examined, {} held (waterlogged), {} run \
         direction(s) at the piece's own outer face (judged at placement by DW0318, not here)",
        binding.examined, binding.held, binding.at_edge
    );
    binding
}

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
    "minecraft:iron_chain",
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

/// "Which ids are air" has one answer in this repo, and it is [`fluid::is_air`]
/// — the module that had to settle the question to say what a body of fluid runs
/// into. A second copy here would be a second authority on the same three ids.
use fluid::is_air;

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
/// **A scatter delivers the quantity it aims at, over the whole domain it was
/// given** — and the number is asserted rather than looked at.
///
/// The owner's cherry-grove finding: a valley staged as a grove held a handful
/// of trees and bare rock in every other direction, and the acceptance proxy was
/// a rendered shot. A shot is one bearing; it cannot see a region, and nothing
/// anywhere produced the count. A scatter that aims at N and stops early because
/// its candidates ran out — every one of them rejected by a spacing rule, an
/// exclusion, or ground it cannot stand on — is silent today: the loop simply
/// ends, and what ships is whatever the ground happened to allow.
///
/// `domain` is the region the generator was given, NOT the subset that turned
/// out to be usable, because a count whose denominator is the part that worked
/// is the render shot in numeric clothes. Both are in the message, so a failure
/// says which of the two ran out.
///
/// The compiler's own `scatter`/`plant` verbs answer the same question through
/// `DW0864`; this is that rule at the layer that has no diagnostics, only
/// panics.
pub fn assert_scatter_reaches_its_target(
    id: &str,
    what: &str,
    want: usize,
    got: usize,
    domain: usize,
    usable: usize,
) {
    assert!(
        got >= want,
        "{id}: the {what} scatter aimed at {want} and placed {got} over a domain of \
         {domain} cell(s), {usable} of which it could use. Widen the domain, relax the \
         spacing, or aim at what the ground can carry — never ship the shortfall, which \
         is what a render shot of one bearing cannot see"
    );
}

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
    WEATHERED
        .iter()
        .find(|(from, _)| *from == name)
        .map(|(_, to)| *to)
}

/// The wear map itself.
///
/// A table rather than a `match` arm set for one reason: a test can walk a
/// table. `every_curated_block_name_is_a_real_block` asserts every key *and*
/// every value here is a block Minecraft 1.21.11 actually has — the claim the
/// doc comment above makes in prose, made checkable. A `match` can only be
/// spot-checked at names somebody thought to write down.
const WEATHERED: &[(&str, &str)] = &[
    ("minecraft:stone", "minecraft:cobblestone"),
    ("minecraft:stone_stairs", "minecraft:cobblestone_stairs"),
    ("minecraft:stone_slab", "minecraft:cobblestone_slab"),
    ("minecraft:cobblestone", "minecraft:mossy_cobblestone"),
    (
        "minecraft:cobblestone_stairs",
        "minecraft:mossy_cobblestone_stairs",
    ),
    (
        "minecraft:cobblestone_slab",
        "minecraft:mossy_cobblestone_slab",
    ),
    (
        "minecraft:cobblestone_wall",
        "minecraft:mossy_cobblestone_wall",
    ),
    ("minecraft:stone_bricks", "minecraft:cracked_stone_bricks"),
    (
        "minecraft:stone_brick_stairs",
        "minecraft:mossy_stone_brick_stairs",
    ),
    (
        "minecraft:stone_brick_slab",
        "minecraft:mossy_stone_brick_slab",
    ),
    (
        "minecraft:stone_brick_wall",
        "minecraft:mossy_stone_brick_wall",
    ),
    ("minecraft:polished_andesite", "minecraft:andesite"),
    (
        "minecraft:polished_andesite_stairs",
        "minecraft:andesite_stairs",
    ),
    (
        "minecraft:polished_andesite_slab",
        "minecraft:andesite_slab",
    ),
    (
        "minecraft:polished_deepslate",
        "minecraft:cobbled_deepslate",
    ),
    (
        "minecraft:polished_deepslate_stairs",
        "minecraft:cobbled_deepslate_stairs",
    ),
    (
        "minecraft:polished_deepslate_slab",
        "minecraft:cobbled_deepslate_slab",
    ),
    (
        "minecraft:deepslate_bricks",
        "minecraft:cracked_deepslate_bricks",
    ),
    (
        "minecraft:deepslate_tiles",
        "minecraft:cracked_deepslate_tiles",
    ),
    ("minecraft:bricks", "minecraft:mud_bricks"),
    ("minecraft:oak_planks", "minecraft:stripped_oak_log"),
    ("minecraft:spruce_planks", "minecraft:stripped_spruce_log"),
];

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

    /// The gate itself: the shipped-and-silent defect, and the id that fixes it.
    #[test]
    #[should_panic(expected = "minecraft:chain is not a block in Minecraft 1.21.11")]
    fn a_block_the_pinned_version_does_not_have_fails() {
        let mut cells = Cells::new();
        cells.insert([0, 0, 0], cell("minecraft:chain"));
        assert_blocks_are_real("fixture", &cells);
    }

    #[test]
    fn the_rename_passes_and_so_does_a_datapacks_own_block() {
        let mut cells = Cells::new();
        cells.insert([0, 0, 0], cell("minecraft:iron_chain"));
        cells.insert([0, 1, 0], cell("delvewright:nonesuch"));
        assert_blocks_are_real("fixture", &cells);
    }

    /// Vanilla drops the whole block state when one property will not parse, so
    /// a bad value is the same class of defect as a bad id.
    #[test]
    #[should_panic(expected = "is not a legal state")]
    fn an_impossible_property_value_fails() {
        let mut props = BTreeMap::new();
        props.insert("facing".to_string(), "up".to_string());
        let mut cells = Cells::new();
        cells.insert([0, 0, 0], ("minecraft:oak_stairs".into(), props));
        assert_blocks_are_real("fixture", &cells);
    }

    /// **Every hand-curated block list in this file is bound to the registry.**
    ///
    /// `TREAD_ATTACHMENT_NAMES` carried `minecraft:chain` — a block that has not
    /// existed since 1.21.11 — which made one of its 23 entries dead code
    /// nothing could ever match. A curated list that names an impossible block
    /// is the same defect as a generator emitting one, one layer up: it is a
    /// belief about the game with nothing checking it. The binding counts are
    /// asserted so a list that shrank to nothing cannot pass quietly.
    #[test]
    fn every_curated_block_name_is_a_real_block() {
        let registry = block_registry();
        for name in TREAD_ATTACHMENT_NAMES {
            assert!(
                registry.contains_key(*name),
                "TREAD_ATTACHMENT_NAMES names {name}, which Minecraft 1.21.11 does not have"
            );
        }
        assert_eq!(TREAD_ATTACHMENT_NAMES.len(), 23);

        // The whole wear map: every surface it is keyed on and every surface it
        // answers with. The doc comment on `weathered` claims "every entry is a
        // real 1.21.11 block"; this is that claim, checked.
        for (from, to) in WEATHERED {
            assert!(
                registry.contains_key(*from),
                "weathered() is keyed on {from}, which Minecraft 1.21.11 does not have"
            );
            assert!(
                registry.contains_key(*to),
                "weathered({from}) is {to}, which Minecraft 1.21.11 does not have"
            );
        }
        assert_eq!(WEATHERED.len(), 22, "the wear map lost entries");
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

    /// A 3x3x3 box of stone with one cell hollowed at the centre, plus whatever
    /// the caller puts in that cell.
    fn walled_box(middle: &[([i32; 3], &str)]) -> Cells {
        let mut cells = Cells::new();
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    cells.insert([x, y, z], cell("minecraft:stone"));
                }
            }
        }
        for (pos, name) in middle {
            cells.insert(*pos, cell(name));
        }
        cells
    }

    /// A source with stone on all five sides it could run into stays put.
    #[test]
    fn a_sealed_pool_is_contained() {
        let cells = walled_box(&[([1, 1, 1], "minecraft:water")]);
        let b = assert_fluid_is_contained("fixture", [3, 3, 3], &cells);
        assert_eq!(b.examined, 1);
        assert_eq!(b.at_edge, 0);
    }

    /// The cave-shore shape: a sea written a block proud of the ground beside
    /// it, so the cell next to it is air.
    #[test]
    #[should_panic(expected = "way(s) out of")]
    fn a_source_beside_air_fails() {
        let mut cells = walled_box(&[([1, 1, 1], "minecraft:water")]);
        cells.insert([2, 1, 1], cell("minecraft:air"));
        assert_fluid_is_contained("fixture", [3, 3, 3], &cells);
    }

    /// A cell the piece never writes is air too — that is what `/place
    /// template` leaves — so the walk cannot look only at written cells.
    #[test]
    #[should_panic(expected = "unwritten, i.e. air")]
    fn a_source_beside_an_unwritten_cell_fails() {
        let mut cells = walled_box(&[([1, 1, 1], "minecraft:water")]);
        cells.remove(&[1, 0, 1]);
        assert_fluid_is_contained("fixture", [3, 3, 3], &cells);
    }

    /// `level` is a value the game derives and re-derives; a piece cannot pin
    /// one, so an authored flow is a defect rather than a state.
    #[test]
    #[should_panic(expected = "mid-flow, not a source")]
    fn an_authored_flow_fails() {
        let mut cells = walled_box(&[]);
        let mut props = BTreeMap::new();
        props.insert("level".into(), "3".into());
        cells.insert([1, 1, 1], ("minecraft:water".into(), props));
        assert_fluid_is_contained("fixture", [3, 3, 3], &cells);
    }

    /// Water at the piece's own outer face is COUNTED, never failed: what is
    /// beyond that face is not in these bytes. The count is what keeps the
    /// trade visible instead of silent.
    #[test]
    fn a_source_on_the_outer_face_is_counted_and_not_failed() {
        let mut cells = walled_box(&[]);
        cells.insert([1, 1, 0], cell("minecraft:water"));
        let b = assert_fluid_is_contained("fixture", [3, 3, 3], &cells);
        assert_eq!(b.examined, 1);
        assert_eq!(b.at_edge, 1, "the -Z run leaves the piece");
    }

    /// A block written `waterlogged=true` is wet, spreads nothing, and is a
    /// wall for anything beside it — the measured fact the shared `fluid`
    /// module carries, reached here rather than restated.
    #[test]
    fn a_waterlogged_block_is_held_and_walls_the_body() {
        let mut cells = walled_box(&[([1, 1, 1], "minecraft:water")]);
        let mut props = BTreeMap::new();
        props.insert("waterlogged".into(), "true".into());
        props.insert("facing".into(), "north".into());
        cells.insert([2, 1, 1], ("minecraft:oak_stairs".into(), props));
        let b = assert_fluid_is_contained("fixture", [3, 3, 3], &cells);
        assert_eq!(b.examined, 1);
        assert_eq!(b.held, 1);
    }
}
