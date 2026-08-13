//! Audit fixtures (spec-0007 quality bar): a clean piece, a command-block piece,
//! a disallowed-palette piece, and an NBT-bearing spawner.

use delvewright_admit::allowlist::Allowlist;
use delvewright_admit::audit::{audit, audit_tile_set};
use delvewright_admit::fixtures;
use delvewright_admit::meta::{License, PrefabMeta};
use delvewright_admit::socket::{self, SocketDecl};
use delvewright_admit::structure::Structure;
use delvewright_schem::split::TilePart;

/// Pair a structure with the manifest entry that would describe it.
fn tile(index: i32, offset: [i32; 3], s: Structure) -> (TilePart, Structure) {
    (
        TilePart {
            file: format!("zone.x0y0z{index}.nbt"),
            id: format!("zone.x0y0z{index}"),
            grid_index: [0, 0, index],
            offset,
            size: s.size,
        },
        s,
    )
}

/// A zone that ships as a tile set is audited tile by tile and judged once.
/// The report is about the zone: its size, its whole block count, the union of
/// its palettes — plus the tile list, so a reader can see how many files the
/// one verdict actually covers.
#[test]
fn a_clean_tile_set_passes_as_one_zone() {
    let a = fixtures::clean_room();
    let b = fixtures::clean_room();
    let (blocks_a, blocks_b) = (a.blocks.len(), b.blocks.len());
    let depth = a.size[2];
    let tiles = vec![tile(0, [0, 0, 0], a), tile(1, [0, 0, depth], b)];
    let (rep, diags) = audit_tile_set(
        "zone.json",
        [tiles[0].1.size[0], tiles[0].1.size[1], depth * 2],
        &tiles,
        &Allowlist::default_building(),
    );

    assert!(rep.is_pass(), "{:?}", rep.findings);
    assert!(diags.is_empty());
    assert_eq!(rep.size[2], depth * 2, "the report sizes the ZONE");
    assert_eq!(rep.block_count, blocks_a + blocks_b);
    assert!(rep.palette.iter().any(|b| b == "minecraft:stone_bricks"));

    let listed = rep.tiles.expect("a zone report names its tiles");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[1].offset, [0, 0, depth]);
    assert!(listed.iter().all(|t| t.verdict == "pass"));
}

/// A forbidden block in the SECOND tile fails the whole zone, and its position
/// is reported in zone coordinates.
///
/// The tile-local coordinate is the trap: it is a real cell of a real file, so
/// it reads as a usable answer, and it points an author at a cell of their
/// design that is innocent. A packaging boundary must not appear in a
/// diagnostic.
#[test]
fn a_forbidden_block_in_a_later_tile_fails_the_zone_at_zone_coordinates() {
    let clean = fixtures::clean_room();
    let depth = clean.size[2];
    let tiles = vec![
        tile(0, [0, 0, 0], clean),
        tile(1, [0, 0, depth], fixtures::command_block_piece()),
    ];
    let (rep, _) = audit_tile_set(
        "zone.json",
        [16, 16, depth * 2],
        &tiles,
        &Allowlist::default_building(),
    );

    assert!(!rep.is_pass(), "a tile is not a unit of judgement");
    assert!(rep.forbidden >= 1);
    let hit = rep
        .findings
        .iter()
        .find(|f| f.code == "DW0731" && f.pos.is_some())
        .expect("the forbidden cell is located");
    assert_eq!(
        hit.pos,
        Some([1, 1, 1 + depth]),
        "the offending cell is at [1,1,1] of tile 1, which is [1,1,{}] of the zone",
        1 + depth
    );

    let listed = rep.tiles.unwrap();
    assert_eq!(listed[0].verdict, "pass");
    assert_eq!(
        listed[1].verdict, "fail",
        "the per-tile verdicts stay visible; it is the ZONE verdict that is binding"
    );
}

#[test]
fn carved_jigsaw_socket_passes_the_audit() {
    // A library prefab's jigsaw sockets are legitimate (the solver mates them) and
    // must NOT be hard-forbidden — only command/structure blocks + NBT spawners are.
    let mut s = fixtures::clean_room();
    let mut meta = PrefabMeta::skeleton(
        "clean",
        s.size,
        s.data_version,
        "delve-admit (external admission)",
        License {
            source: "original".into(),
            spdx: "CC0-1.0".into(),
            note: String::new(),
            provenance: String::new(),
            generated_by: None,
        },
    );
    socket::carve(&mut s, &mut meta, &SocketDecl::new([3, 1, 0], "north")).unwrap();
    let (rep, _) = audit("socketed", &s, &Allowlist::default_building());
    assert!(
        rep.is_pass(),
        "carved socket should pass: {:?}",
        rep.findings
    );
    assert!(s.block_names().contains("minecraft:jigsaw"));
}

#[test]
fn clean_piece_passes() {
    let s = fixtures::clean_room();
    let (rep, diags) = audit("clean", &s, &Allowlist::default_building());
    assert!(rep.is_pass(), "clean room should pass: {:?}", rep.findings);
    assert_eq!(rep.forbidden, 0);
    assert_eq!(rep.not_allowlisted, 0);
    assert!(diags.is_empty());
    // palette is reported for reviewer visibility.
    assert!(rep.palette.iter().any(|b| b == "minecraft:stone_bricks"));
}

#[test]
fn command_block_piece_is_hard_rejected() {
    let s = fixtures::command_block_piece();
    let (rep, _) = audit("cb", &s, &Allowlist::default_building());
    assert!(!rep.is_pass());
    assert!(rep.forbidden >= 1, "command block must be forbidden");
    // both the block name and the embedded Command are DW0731.
    assert!(rep.findings.iter().any(|f| f.code == "DW0731"));
    // the offending cell position is reported.
    assert!(rep.findings.iter().any(|f| f.pos == Some([1, 1, 1])));
}

#[test]
fn spawner_with_nbt_is_hard_rejected() {
    let s = fixtures::spawner_piece();
    let (rep, _) = audit("sp", &s, &Allowlist::default_building());
    assert!(!rep.is_pass());
    assert!(rep.findings.iter().any(|f| f.code == "DW0731"));
}

#[test]
fn disallowed_palette_block_fails_allowlist() {
    let s = fixtures::disallowed_palette_piece();
    let (rep, _) = audit("tnt", &s, &Allowlist::default_building());
    assert!(!rep.is_pass());
    assert_eq!(rep.forbidden, 0, "tnt is not a code-injection vector");
    assert!(rep.not_allowlisted >= 1);
    assert!(rep.findings.iter().any(|f| f.code == "DW0730"));
}

#[test]
fn allowlist_override_can_admit_a_surprising_block() {
    let s = fixtures::disallowed_palette_piece();
    let allow = Allowlist::from_file(r#"{ "allow": ["minecraft:air","minecraft:stone_bricks","minecraft:glowstone","minecraft:tnt"] }"#).unwrap();
    let (rep, _) = audit("tnt", &s, &allow);
    assert!(
        rep.is_pass(),
        "override should admit tnt: {:?}",
        rep.findings
    );
}

#[test]
fn report_json_is_machine_readable() {
    let s = fixtures::command_block_piece();
    let (rep, _) = audit("cb", &s, &Allowlist::default_building());
    let json = rep.to_json();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["verdict"], "fail");
    assert!(!v["findings"].as_array().unwrap().is_empty());
    assert!(json.ends_with("}\n"));
}

/// `DW0733`: a block the pinned game does not have.
///
/// The allowlist cannot catch this and the test says why: `minecraft:chain` is
/// in the built-in allowlist to this day, because an allowlist is a list of
/// names somebody once approved and nothing re-checks a name against the game.
#[test]
fn a_block_the_pinned_version_does_not_have_fails_the_audit() {
    let s = fixtures::renamed_block_piece();
    let (rep, _) = audit("ropes", &s, &Allowlist::default_building());
    assert!(!rep.is_pass());
    assert_eq!(rep.unknown_blocks, 1);
    assert_eq!(
        rep.forbidden, 0,
        "a renamed block is not an injection vector"
    );
    let f = rep
        .findings
        .iter()
        .find(|f| f.code == "DW0733")
        .expect("DW0733 must fire");
    assert!(f.message.contains("minecraft:iron_chain"), "{}", f.message);
    assert_eq!(f.severity, "error");
}

/// `DW0734`: a state the pinned game *does* have, written so that only the
/// pinned game can read it.
///
/// This is the half `DW0733` structurally cannot reach. `DW0733` asks "is every
/// property you wrote legal", and a palette entry that writes none passes that
/// question trivially — a bare `minecraft:cobblestone_wall` is a perfectly legal
/// state. What it is not is a *stated* one: vanilla resolves it from the block's
/// default state, and the prefab viewer, having no defaults, could satisfy no
/// `multipart` case, unioned all of them, and drew a solid 1×1×1 cube where a
/// wall post stands — over a report that said `0 unresolved`.
#[test]
fn a_state_that_does_not_say_what_it_is_fails_the_audit() {
    let s = fixtures::under_specified_block_piece();
    let (rep, _) = audit("meadow-wall", &s, &Allowlist::default_building());
    assert!(!rep.is_pass());
    assert_eq!(rep.under_specified_blocks, 1);
    assert_eq!(
        rep.unknown_blocks, 0,
        "the state is legal — this is exactly what DW0733 cannot see"
    );
    let f = rep
        .findings
        .iter()
        .find(|f| f.code == "DW0734")
        .expect("DW0734 must fire");
    assert_eq!(f.severity, "error");
    // The message names what vanilla would have filled in, so the fix is the
    // message: a wall post with an `up` column and no arms.
    assert!(f.message.contains("up=true"), "{}", f.message);
    assert!(f.message.contains("north=none"), "{}", f.message);
}

/// ...and the same wall, written out, audits clean. The BlockState is identical
/// — completeness is about what the file says, not about what the world is — so
/// the gate must accept it, or it would be refusing the fix it prescribes.
#[test]
fn the_same_wall_written_out_passes_the_audit() {
    let mut s = fixtures::clean_room();
    s.set_cell(
        [2, 1, 2],
        delvewright_admit::structure::PaletteEntry::with_props(
            "minecraft:cobblestone_wall",
            &[
                ("east", "none"),
                ("north", "none"),
                ("south", "none"),
                ("up", "true"),
                ("waterlogged", "false"),
                ("west", "none"),
            ],
        ),
        None,
    );
    let (rep, _) = audit("meadow-wall", &s, &Allowlist::default_building());
    assert_eq!(rep.under_specified_blocks, 0);
    assert!(rep.is_pass(), "{:?}", rep.findings);
}

/// A template saved by an OLDER game is not this registry's business. Vanilla's
/// DataFixerUpper upgrades a pre-pin palette on load, and the pinned registry
/// describes 1.21.11 only, so it cannot say what a 1.18.2 entry left out. The
/// three third-party pieces in the library are all pre-pin
/// (`hero-temple-ruin-arch` at 2975, `hero-galleon-oak` at 3465,
/// `hero-standing-monolith` at 4189); refusing them here would be a verdict the
/// data cannot support.
#[test]
fn a_pre_pin_template_is_not_judged_for_completeness() {
    let mut s = fixtures::under_specified_block_piece();
    s.data_version = 2975; // MC 1.18.2
    let (rep, _) = audit("moss-ruins", &s, &Allowlist::default_building());
    assert_eq!(rep.under_specified_blocks, 0);
    assert!(rep.is_pass(), "{:?}", rep.findings);
}

/// ...and the rename itself audits clean, so the gate is not simply refusing
/// everything in that shape.
#[test]
fn the_rename_passes_the_audit() {
    let mut s = fixtures::clean_room();
    s.set_cell(
        [2, 1, 2],
        // Written out: `iron_chain` has an `axis` and a `waterlogged`, and a
        // fixture that leaves them for the game to fill in is the same lossy
        // shape the library shipped (`DW0734`).
        delvewright_admit::structure::PaletteEntry::with_props(
            "minecraft:iron_chain",
            &[("axis", "y"), ("waterlogged", "false")],
        ),
        None,
    );
    let (rep, _) = audit("ropes", &s, &Allowlist::default_building());
    assert_eq!(rep.unknown_blocks, 0);
    assert!(rep.is_pass(), "{:?}", rep.findings);
}
