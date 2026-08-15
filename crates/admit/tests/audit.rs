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

/// `DW0733`: a block the pinned game does not have, in a template at the pin.
///
/// The allowlist cannot catch this class and the test says why: an allowlist
/// is a list of names somebody once approved, and nothing re-checks a name
/// against the game — `minecraft:chain` sat in the built-in allowlist long
/// after 1.21.11 renamed it (the entry is `iron_chain` now, but that was a
/// hand edit, which is exactly what cannot be relied on).
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

/// ...and the rename itself audits clean, so the gate is not simply refusing
/// everything in that shape.
#[test]
fn the_rename_passes_the_audit() {
    let mut s = fixtures::clean_room();
    s.set_cell(
        [2, 1, 2],
        delvewright_admit::structure::PaletteEntry::simple("minecraft:iron_chain"),
        None,
    );
    let (rep, _) = audit("ropes", &s, &Allowlist::default_building());
    assert_eq!(rep.unknown_blocks, 0);
    assert!(rep.is_pass(), "{:?}", rep.findings);
}

/// `DW0734`: the same unknown id in a template that PRE-DATES the pin is not a
/// defect — the game datafixes every structure it loads against the file's
/// `DataVersion`, and `chain` becomes `iron_chain`. The shipped proof is
/// `prefabs/hero-temple-ruin-arch.nbt` (DataVersion 2975, carries
/// `minecraft:chain`, loads fine); refusing it under `DW0733` was a measured
/// false positive. The audit warns — loud enough to catch a typo no fixer will
/// ever map — and passes.
///
/// The allowlist is the DEFAULT one on purpose. This test used to hand it a
/// bespoke list containing `minecraft:chain`, "so the test isolates the
/// DataVersion rule" — and that workaround was the whole defect in miniature:
/// the default list does not contain `minecraft:chain` (it contains
/// `minecraft:iron_chain`, which is the block the server holds), so the audit
/// warned in one breath and refused the identical cell under `DW0730` in the
/// next. Nothing here isolates that any more; the two rules have to agree.
#[test]
fn a_pre_pin_template_with_a_datafixable_id_warns_instead_of_failing() {
    let mut s = fixtures::renamed_block_piece();
    s.data_version = 2975; // hero-temple-ruin-arch's actual DataVersion
    let (rep, _) = audit("arch", &s, &Allowlist::default_building());
    assert!(rep.is_pass(), "{:?}", rep.findings);
    assert_eq!(rep.unknown_blocks, 0);
    assert_eq!(rep.pre_pin_unknown, 1);
    assert_eq!(rep.not_allowlisted, 0);
    let f = rep
        .findings
        .iter()
        .find(|f| f.code == "DW0734")
        .expect("DW0734 must fire");
    assert_eq!(f.severity, "warning");
    assert!(f.message.contains("2975"), "{}", f.message);
    // The warning names the id the server will hold, not only the one it will
    // not: a reader who has to guess which of three chains the fixer picks has
    // not been told anything.
    assert!(f.message.contains("minecraft:iron_chain"), "{}", f.message);
}

/// `DW0730` over the id the game LOADS, in both directions.
///
/// The allowlist is a list of names at the pin, so a pre-pin palette must be
/// resolved before it is judged — but resolving it must not become a way in.
/// Both halves are asserted here, because a fix that only made the arch pass
/// would be indistinguishable from widening the list.
#[test]
fn the_allowlist_judges_the_id_the_game_loads_and_still_refuses_a_dead_one() {
    // 1. The renamed id resolves to an allowlisted block: admitted.
    let mut arch = fixtures::renamed_block_piece();
    arch.data_version = 2975;
    let (rep, _) = audit("arch", &arch, &Allowlist::default_building());
    assert_eq!(rep.not_allowlisted, 0, "{:?}", rep.findings);

    // 2. The SAME id at the pin resolves to nothing — no fixer runs — so it is
    //    still refused, and by both rules.
    let mut at_pin = fixtures::renamed_block_piece();
    at_pin.data_version = delvewright_schem::blocks::PIN_DATA_VERSION;
    let (rep, _) = audit("bell-tower", &at_pin, &Allowlist::default_building());
    assert!(!rep.is_pass());
    assert_eq!(rep.unknown_blocks, 1);
    assert_eq!(rep.not_allowlisted, 1, "{:?}", rep.findings);

    // 3. A pre-pin id nothing renames stays refused: resolution is not a pass.
    let mut typo = fixtures::clean_room();
    typo.data_version = 2975;
    typo.set_cell(
        [2, 1, 2],
        delvewright_admit::structure::PaletteEntry::simple("minecraft:chian"),
        None,
    );
    let (rep, _) = audit("typo", &typo, &Allowlist::default_building());
    assert!(!rep.is_pass());
    assert_eq!(rep.not_allowlisted, 1, "{:?}", rep.findings);

    // 4. A pre-pin rename whose TARGET the allowlist does not permit is refused
    //    on the target, and the diagnostic names both ids — otherwise the
    //    reviewer is sent to look for a block the file does not contain.
    let narrow = Allowlist::from_file(
        r#"{ "allow": ["minecraft:air", "minecraft:stone_bricks", "minecraft:glowstone"] }"#,
    )
    .unwrap();
    let (rep, _) = audit("arch", &arch, &narrow);
    assert!(!rep.is_pass());
    assert_eq!(rep.not_allowlisted, 1);
    let f = rep
        .findings
        .iter()
        .find(|f| f.code == "DW0730")
        .expect("DW0730 must fire");
    assert!(f.message.contains("minecraft:iron_chain"), "{}", f.message);
    assert!(f.message.contains("minecraft:chain"), "{}", f.message);
    assert!(f.message.contains("2975"), "{}", f.message);
}

/// `DW0735`: a shape-carrying (multipart) property omitted is an error — the
/// block places disconnected — while a variant-picking omission (a lantern's
/// `hanging`, `waterlogged` anywhere) is the author's default and stays
/// silent. The line is the block class's own blockstate definition, not a
/// hand-kept list.
#[test]
fn an_omitted_connection_property_fails_and_a_variant_omission_does_not() {
    let mut s = fixtures::clean_room();
    // The real defect: bars with nothing written — an isolated post.
    s.set_cell(
        [2, 1, 2],
        delvewright_admit::structure::PaletteEntry::simple("minecraft:iron_bars"),
        None,
    );
    // The benign omission: a lantern with neither `hanging` nor `waterlogged`.
    s.set_cell(
        [1, 2, 1],
        delvewright_admit::structure::PaletteEntry::simple("minecraft:lantern"),
        None,
    );
    let (rep, _) = audit("grate", &s, &Allowlist::default_building());
    assert!(!rep.is_pass());
    assert_eq!(rep.underspecified, 1, "the lantern must not be flagged");
    let f = rep
        .findings
        .iter()
        .find(|f| f.code == "DW0735")
        .expect("DW0735 must fire");
    assert_eq!(f.severity, "error");
    assert!(f.message.contains("minecraft:iron_bars"), "{}", f.message);
    assert!(
        f.message.contains("east, north, south, west"),
        "{}",
        f.message
    );

    // ...and the same bars with their connections written audit clean.
    let mut s = fixtures::clean_room();
    s.set_cell(
        [2, 1, 2],
        delvewright_admit::structure::PaletteEntry::with_props(
            "minecraft:iron_bars",
            &[
                ("east", "false"),
                ("north", "true"),
                ("south", "true"),
                ("west", "false"),
            ],
        ),
        None,
    );
    let (rep, _) = audit("grate", &s, &Allowlist::default_building());
    assert_eq!(rep.underspecified, 0);
    assert!(rep.is_pass(), "{:?}", rep.findings);
}
