//! **The support table against a second method**: the Minecraft Wiki's
//! per-block placement statements, for a sample of blocks, read as facts and
//! asserted against `delvewright_dsl::support` — which was measured by asking
//! the pinned jar's `canSurvive` (`tools/maintenance/dump-support.py`) and
//! shares no configuration with a wiki page. A disagreement is either a table
//! defect or a page that does not describe 1.21.11; it is read before either is
//! believed.
//!
//! Each case names the page and section it is taken from (minecraft.wiki; the
//! statements are paraphrased facts, not quotations — the wiki's text is not
//! under a licence this repository adopts, ADR-0013).

use delvewright_dsl::blockshape::Face;
use delvewright_dsl::support::needs;

/// Whether `block` stays with `beside` at `cell` and air everywhere else.
fn kept(block: &str, cell: Face, beside: &str) -> bool {
    needs(block)
        .unwrap_or_else(|| panic!("{block} is in the table"))
        .kept(|f| if f == cell { beside } else { "minecraft:air" })
        .unwrap_or_else(|| panic!("{block} is judged"))
}

fn check(page: &str, block: &str, cell: Face, cases: &[(&str, bool)]) {
    for (beside, wiki) in cases {
        assert_eq!(
            kept(block, cell, beside),
            *wiki,
            "{page}: `{block}` with `{beside}` {} it — the wiki says {}",
            match cell {
                Face::Down => "under",
                Face::Up => "over",
                _ => "beside",
            },
            if *wiki { "it stays" } else { "it does not" }
        );
    }
}

/// *Torch* — History: a torch stands on fences (1.7), glass (12w07a), glass
/// panes and iron bars (1.14.3-pre3), upside-down slabs and stairs (12w25a);
/// it is not placed on leaves (1.1.0). Usage: on top of most solid blocks.
#[test]
fn torch() {
    check(
        "Torch",
        "minecraft:torch",
        Face::Down,
        &[
            ("minecraft:stone", true),
            ("minecraft:oak_fence", true),
            ("minecraft:glass", true),
            ("minecraft:glass_pane", true),
            ("minecraft:iron_bars", true),
            ("minecraft:stone_slab[type=top]", true),
            ("minecraft:oak_stairs[half=top]", true),
            ("minecraft:oak_leaves", false),
            ("minecraft:air", false),
        ],
    );
}

/// *Lantern* — Usage: placed on top of or hung from most solid blocks; hung
/// from a chain's bottom face. History 1.14.3-pre3: placed and hung on iron
/// bars and glass panes. Trivia: a turtle egg holds one above and below (half
/// of which the jar contradicts — see the case).
#[test]
fn lantern() {
    let standing = "minecraft:lantern[hanging=false]";
    let hanging = "minecraft:lantern[hanging=true]";
    for beside in [
        "minecraft:stone",
        "minecraft:iron_bars",
        "minecraft:glass_pane",
    ] {
        check("Lantern", standing, Face::Down, &[(beside, true)]);
        check("Lantern", hanging, Face::Up, &[(beside, true)]);
    }
    // The one disagreement of the sample, read and settled for the jar: the
    // Trivia line says a turtle egg holds a lantern above it too. In the pinned
    // jar a standing lantern asks `Block.canSupportCenter` of the block under
    // it, whose UP face shape must cover the centre column; a turtle egg's box
    // tops out at 7/16, so its UP face shape is empty and the answer is no. Its
    // DOWN face is the egg's base, which covers the centre, so it does hold a
    // hanging one. The line does not describe 1.21.11 Java on the first half.
    check(
        "Lantern",
        hanging,
        Face::Up,
        &[("minecraft:turtle_egg", true)],
    );
    check(
        "Lantern",
        standing,
        Face::Down,
        &[("minecraft:turtle_egg", false)],
    );
    check(
        "Lantern",
        hanging,
        Face::Up,
        &[("minecraft:iron_chain", true), ("minecraft:air", false)],
    );
    check("Lantern", standing, Face::Down, &[("minecraft:air", false)]);
}

/// *Rail* — Usage > Placement: on a block whose top face has a rim — any full
/// block, a hopper, a top slab, upside-down stairs, a top trapdoor; never on a
/// side. History 19w12b: on glass, ice, glowstone and sea lanterns.
#[test]
fn rail() {
    check(
        "Rail",
        "minecraft:rail",
        Face::Down,
        &[
            ("minecraft:stone", true),
            ("minecraft:hopper", true),
            ("minecraft:stone_slab[type=top]", true),
            ("minecraft:oak_stairs[half=top]", true),
            ("minecraft:oak_trapdoor[half=top]", true),
            ("minecraft:glass", true),
            ("minecraft:ice", true),
            ("minecraft:glowstone", true),
            ("minecraft:sea_lantern", true),
            ("minecraft:stone_slab[type=bottom]", false),
            ("minecraft:air", false),
        ],
    );
    check(
        "Rail",
        "minecraft:rail",
        Face::North,
        &[("minecraft:stone", false)],
    );
}

/// *Ladder* — Usage > Placement: on the side of any full solid block, not on a
/// top or bottom face. History: on glowstone (19w13a), on blocks of redstone,
/// observers and targets (20w10a). `ladder[facing=north]` hangs on the block
/// south of it.
#[test]
fn ladder() {
    let l = "minecraft:ladder[facing=north]";
    check(
        "Ladder",
        l,
        Face::South,
        &[
            ("minecraft:stone", true),
            ("minecraft:glowstone", true),
            ("minecraft:redstone_block", true),
            ("minecraft:observer", true),
            ("minecraft:target", true),
            ("minecraft:air", false),
        ],
    );
    check("Ladder", l, Face::Down, &[("minecraft:stone", false)]);
}

/// *Carpet* — Usage: on any block but air, non-solid ones included, fences and
/// walls named, and over water.
#[test]
fn carpet() {
    check(
        "Carpet",
        "minecraft:white_carpet",
        Face::Down,
        &[
            ("minecraft:stone", true),
            ("minecraft:oak_fence", true),
            ("minecraft:cobblestone_wall", true),
            ("minecraft:water", true),
            ("minecraft:air", false),
        ],
    );
}

/// *Flower* — Usage: most flowers are planted on grass blocks, dirt, coarse
/// dirt, rooted dirt, farmland, podzol, mycelium, moss blocks, pale moss
/// blocks, mud and muddy mangrove roots — stone is not among them.
#[test]
fn flower() {
    let soil = [
        "minecraft:grass_block",
        "minecraft:dirt",
        "minecraft:coarse_dirt",
        "minecraft:rooted_dirt",
        "minecraft:farmland",
        "minecraft:podzol",
        "minecraft:mycelium",
        "minecraft:moss_block",
        "minecraft:pale_moss_block",
        "minecraft:mud",
        "minecraft:muddy_mangrove_roots",
    ];
    for s in soil {
        check("Flower", "minecraft:poppy", Face::Down, &[(s, true)]);
    }
    check(
        "Flower",
        "minecraft:poppy",
        Face::Down,
        &[("minecraft:stone", false), ("minecraft:sand", false)],
    );
}
