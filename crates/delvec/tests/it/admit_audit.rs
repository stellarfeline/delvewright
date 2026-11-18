//! Audit fixtures (spec-0007 quality bar): a clean piece, a command-block piece,
//! and an NBT-bearing spawner.

use delvec::admit::audit::{audit, audit_tile_set};
use delvec::admit::fixtures;
use delvec::admit::meta::{License, PrefabMeta};
use delvec::admit::socket::{self, SocketDecl};
use delvec::admit::structure::Structure;
use delvec::schem::split::TilePart;

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
    let (rep, _) = audit_tile_set("zone.json", [16, 16, depth * 2], &tiles);

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
        "delvec prefab (external admission)",
        License {
            source: "original".into(),
            spdx: "CC0-1.0".into(),
            note: String::new(),
            provenance: String::new(),
            generated_by: None,
        },
    );
    socket::carve(&mut s, &mut meta, &SocketDecl::new([3, 1, 0], "north")).unwrap();
    let (rep, _) = audit("socketed", &s);
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
    let (rep, diags) = audit("clean", &s);
    assert!(rep.is_pass(), "clean room should pass: {:?}", rep.findings);
    assert_eq!(rep.forbidden, 0);
    assert!(diags.is_empty());
    // palette is reported for reviewer visibility.
    assert!(rep.palette.iter().any(|b| b == "minecraft:stone_bricks"));
}

#[test]
fn command_block_piece_is_hard_rejected() {
    let s = fixtures::command_block_piece();
    let (rep, _) = audit("cb", &s);
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
    let (rep, _) = audit("sp", &s);
    assert!(!rep.is_pass());
    assert!(rep.findings.iter().any(|f| f.code == "DW0731"));
}

#[test]
fn report_json_is_machine_readable() {
    let s = fixtures::command_block_piece();
    let (rep, _) = audit("cb", &s);
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
    let (rep, _) = audit("ropes", &s);
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
        delvec::admit::structure::PaletteEntry::simple("minecraft:iron_chain"),
        None,
    );
    let (rep, _) = audit("ropes", &s);
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
#[test]
fn a_pre_pin_template_with_a_datafixable_id_warns_instead_of_failing() {
    let mut s = fixtures::renamed_block_piece();
    s.data_version = 2975; // hero-temple-ruin-arch's actual DataVersion
    let (rep, _) = audit("arch", &s);
    assert!(rep.is_pass(), "{:?}", rep.findings);
    assert_eq!(rep.unknown_blocks, 0);
    assert_eq!(rep.pre_pin_unknown, 1);
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

/// **Resolving a pre-pin id is not a way in.** A renamed id is admitted on
/// the id the game loads; the same id at the pin is refused (`DW0733`), and a
/// pre-pin id no derived rename reaches is a dead id, refused under `DW0733`
/// rather than warned about — it loads as air.
#[test]
fn a_dead_id_is_refused_and_a_renamed_one_admitted() {
    // 1. The renamed id resolves to a block the pin holds: admitted.
    let mut arch = fixtures::renamed_block_piece();
    arch.data_version = 2975;
    let (rep, _) = audit("arch", &arch);
    assert!(rep.is_pass(), "{:?}", rep.findings);

    // 2. The SAME id at the pin resolves to nothing — no fixer runs.
    let mut at_pin = fixtures::renamed_block_piece();
    at_pin.data_version = delvec::schem::blocks::PIN_DATA_VERSION;
    let (rep, _) = audit("bell-tower", &at_pin);
    assert!(!rep.is_pass());
    assert_eq!(rep.unknown_blocks, 1, "{:?}", rep.findings);

    // 3. A pre-pin id nothing renames stays refused: resolution is not a pass.
    let mut typo = fixtures::clean_room();
    typo.data_version = 2975;
    typo.set_cell(
        [2, 1, 2],
        delvec::admit::structure::PaletteEntry::simple("minecraft:chian"),
        None,
    );
    let (rep, _) = audit("typo", &typo);
    assert!(!rep.is_pass());
    assert_eq!((rep.unknown_blocks, rep.pre_pin_unknown), (1, 0));
    let f = rep
        .findings
        .iter()
        .find(|f| f.code == "DW0733")
        .expect("DW0733 must fire");
    assert!(f.message.contains("minecraft:chian"), "{}", f.message);
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
        delvec::admit::structure::PaletteEntry::simple("minecraft:iron_bars"),
        None,
    );
    // The benign omission: a lantern with neither `hanging` nor `waterlogged`.
    s.set_cell(
        [1, 2, 1],
        delvec::admit::structure::PaletteEntry::simple("minecraft:lantern"),
        None,
    );
    let (rep, _) = audit("grate", &s);
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
        delvec::admit::structure::PaletteEntry::with_props(
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
    let (rep, _) = audit("grate", &s);
    assert_eq!(rep.underspecified, 0);
    assert!(rep.is_pass(), "{:?}", rep.findings);
}

/// **A merchant's anvil and a trap's trapped chest are building material**,
/// every damage stage of the anvil included.
#[test]
fn an_anvil_and_a_trapped_chest_pass_the_audit() {
    let admitted = [
        "minecraft:anvil",
        "minecraft:chipped_anvil",
        "minecraft:damaged_anvil",
        "minecraft:trapped_chest",
    ];
    let mut s = fixtures::clean_room();
    for (i, id) in admitted.iter().enumerate() {
        s.set_cell(
            [1 + i as i32 % 2, 1, 1 + i as i32 / 2],
            delvec::admit::structure::PaletteEntry::simple(*id),
            None,
        );
    }
    let (rep, _) = audit("forge", &s);
    assert!(rep.is_pass(), "{:?}", rep.findings);
    for id in admitted {
        assert!(rep.palette.iter().any(|b| b == id), "{id} was not examined");
    }
}

/// **The sculk family passes at rest** (spec-0100 §3.1, acceptance criterion
/// 3): a piece carrying all six sculk blocks in their rest states is admitted,
/// and the same piece with one state off rest is refused by
/// the one rest rule — `can_summon=true` as `DW0998`, a sensor caught
/// mid-click as `DW0999` — as is a cell whose block entity carries a pending
/// vibration. Each id is asserted in the report's palette.
#[test]
fn the_sculk_family_passes_at_rest() {
    use delvec::admit::structure::PaletteEntry;
    use delvec::schem::nbt::Nbt;
    let vein = [
        ("down", "true"),
        ("east", "false"),
        ("north", "false"),
        ("south", "false"),
        ("up", "false"),
        ("waterlogged", "false"),
        ("west", "false"),
    ];
    let piece = |shrieker: &[(&str, &str)], sensor: &[(&str, &str)], nbt: Option<Nbt>| {
        let mut s = fixtures::clean_room();
        s.set_cell([1, 1, 1], PaletteEntry::simple("minecraft:sculk"), None);
        s.set_cell(
            [2, 1, 1],
            PaletteEntry::with_props("minecraft:sculk_vein", &vein),
            None,
        );
        s.set_cell(
            [3, 1, 1],
            PaletteEntry::with_props("minecraft:sculk_sensor", sensor),
            nbt,
        );
        s.set_cell(
            [1, 1, 2],
            PaletteEntry::with_props(
                "minecraft:calibrated_sculk_sensor",
                &[
                    ("facing", "north"),
                    ("power", "0"),
                    ("sculk_sensor_phase", "inactive"),
                    ("waterlogged", "false"),
                ],
            ),
            None,
        );
        s.set_cell(
            [2, 1, 2],
            PaletteEntry::with_props("minecraft:sculk_shrieker", shrieker),
            None,
        );
        s.set_cell(
            [3, 1, 2],
            PaletteEntry::with_props("minecraft:sculk_catalyst", &[("bloom", "false")]),
            None,
        );
        s
    };
    let rest_shrieker = [
        ("can_summon", "false"),
        ("shrieking", "false"),
        ("waterlogged", "false"),
    ];
    let rest_sensor = [
        ("power", "0"),
        ("sculk_sensor_phase", "inactive"),
        ("waterlogged", "false"),
    ];
    let (rep, _) = audit("listening", &piece(&rest_shrieker, &rest_sensor, None));
    assert!(rep.is_pass(), "{:?}", rep.findings);
    for id in [
        "minecraft:sculk",
        "minecraft:sculk_vein",
        "minecraft:sculk_sensor",
        "minecraft:calibrated_sculk_sensor",
        "minecraft:sculk_shrieker",
        "minecraft:sculk_catalyst",
    ] {
        assert!(rep.palette.iter().any(|b| b == id), "{id} was not examined");
    }

    let summoning = [
        ("can_summon", "true"),
        ("shrieking", "false"),
        ("waterlogged", "false"),
    ];
    let (rep, _) = audit("listening", &piece(&summoning, &rest_sensor, None));
    assert!(!rep.is_pass());
    assert!(
        rep.findings.iter().any(|f| f.code == "DW0998"),
        "{:?}",
        rep.findings
    );

    let clicking = [
        ("power", "0"),
        ("sculk_sensor_phase", "active"),
        ("waterlogged", "false"),
    ];
    let (rep, _) = audit("listening", &piece(&rest_shrieker, &clicking, None));
    assert!(
        rep.findings
            .iter()
            .any(|f| f.code == "DW0999" && f.message.contains("sculk_sensor_phase")),
        "{:?}",
        rep.findings
    );

    // A sensor saved with a vibration in flight replays it on load.
    let mut listener = std::collections::BTreeMap::new();
    listener.insert("event".to_string(), Nbt::Compound(Default::default()));
    let mut be = std::collections::BTreeMap::new();
    be.insert(
        "id".to_string(),
        Nbt::String("minecraft:sculk_sensor".into()),
    );
    be.insert("listener".to_string(), Nbt::Compound(listener));
    let (rep, _) = audit(
        "listening",
        &piece(&rest_shrieker, &rest_sensor, Some(Nbt::Compound(be))),
    );
    assert!(
        rep.findings
            .iter()
            .any(|f| f.code == "DW0999" && f.message.contains("nbt.listener.event")),
        "{:?}",
        rep.findings
    );
}

/// **Terrain surface blocks pass**: a dirt path, mycelium and farmland.
#[test]
fn dirt_path_mycelium_and_farmland_pass() {
    use delvec::admit::structure::PaletteEntry;
    let admitted = [
        "minecraft:dirt_path",
        "minecraft:mycelium",
        "minecraft:farmland",
    ];
    let mut s = fixtures::clean_room();
    for (i, id) in admitted.iter().enumerate() {
        s.set_cell([1 + i as i32, 1, 1], PaletteEntry::simple(*id), None);
    }
    let (rep, _) = audit("surface", &s);
    assert!(rep.is_pass(), "{:?}", rep.findings);
    for id in admitted {
        assert!(rep.palette.iter().any(|b| b == id), "{id} was not examined");
    }
}

/// **Every container kind a delve furnishes passes the audit**, enumerated
/// from the container table (spec-0105 §2.1): no block id behind the kinds is
/// on the code-injection forbid list, and a piece standing a brewing stand —
/// which a `loot` fill reaches and the skill furnishes — passes, written with
/// the bottle properties its multipart model is assembled from (DW0735
/// otherwise).
#[test]
fn every_container_kind_passes_the_audit() {
    let mut ids = 0;
    for k in delvewright_dsl::blocks::CONTAINER_KINDS_1_21_11 {
        for id in k.blocks {
            ids += 1;
            assert!(
                !delvec::admit::audit::hard_forbidden_block(id),
                "{id} is forbidden"
            );
        }
    }
    assert!(ids >= 12, "container ids examined: {ids}");
    use delvec::admit::structure::PaletteEntry;
    let bottles = [
        ("has_bottle_0", "false"),
        ("has_bottle_1", "false"),
        ("has_bottle_2", "false"),
    ];
    let mut s = fixtures::clean_room();
    s.set_cell(
        [1, 1, 1],
        PaletteEntry::with_props("minecraft:brewing_stand", &bottles),
        None,
    );
    let (rep, _) = audit("still-room", &s);
    assert!(rep.is_pass(), "{:?}", rep.findings);
    assert!(rep.palette.iter().any(|b| b == "minecraft:brewing_stand"));
}
