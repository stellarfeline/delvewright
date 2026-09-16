//! **Furniture is not floor** (spec-0065): the byte claim a furniture
//! declaration makes, the walk model's refusal to stand a body on it, and the
//! checks it must not reach.
//!
//! Every build case here is the hello-world campaign over a hall synthesised in
//! this file and written under `CARGO_TARGET_TMPDIR`, so the table the proofs
//! reason about is a table these tests laid, and the content library is never
//! read or written. The hall keeps hello-room's names and places — the entry at
//! `spawn`, the keeper's stand, the barred door, the exit — and adds a laid table
//! (an `oak_fence` row under an `oak_slab[type=bottom]` top, a `spruce_slab`
//! bench on either side: the released castle's own construction) across the far
//! half, between the door and the exit.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::admit::structure::{PaletteEntry, Structure, synth};
use delvec::compiler::claims::{self, ClaimKey};
use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::prefab::{AnchorRole, PrefabMeta};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

/// Hall extent: 11 wide, 6 tall, 13 deep.
const SIZE: [i32; 3] = [11, 6, 13];
/// The table's row, the benches either side of it, and the exit beyond.
const TABLE_Z: i32 = 9;
const EXIT: [i32; 3] = [5, 1, 11];
/// The side table in the near half — two legs, a top and a bench either side —
/// where a body can be posted without a gate between it and the entry.
const SIDE_TABLE: ([i32; 3], [i32; 3]) = ([8, 1, 3], [9, 2, 3]);
/// A cell a body would stand in ON the side table's top.
const TABLE_TOP_CELL: [i32; 3] = [8, 3, 3];

/// How far the table runs along x. `Wall` spans the hall wall to wall, so the
/// only way from the door to the exit is over it; `Gap` stops two cells short
/// of the east wall, so there is a way round.
#[derive(Clone, Copy)]
enum Table {
    Wall,
    Gap,
}

impl Table {
    fn x_range(self) -> std::ops::RangeInclusive<i32> {
        match self {
            Table::Wall => 1..=9,
            Table::Gap => 1..=7,
        }
    }
}

/// The hall's blocks.
fn hall(table: Table) -> Structure {
    let [sx, sy, sz] = SIZE;
    let mut cells = Vec::new();
    let stone = || PaletteEntry::simple("minecraft:stone");
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let shell = y == 0 || y == sy - 1 || x == 0 || x == sx - 1 || z == 0 || z == sz - 1;
                let divider = z == 6 && (1..sy - 1).contains(&y);
                if shell {
                    cells.push(([x, y, z], stone(), None));
                } else if divider {
                    if (4..=5).contains(&x) && (1..=3).contains(&y) {
                        cells.push(([x, y, z], PaletteEntry::simple("minecraft:iron_bars"), None));
                    } else {
                        cells.push(([x, y, z], stone(), None));
                    }
                }
            }
        }
    }
    for x in table.x_range() {
        cells.push((
            [x, 1, TABLE_Z],
            PaletteEntry::simple("minecraft:oak_fence"),
            None,
        ));
        cells.push((
            [x, 2, TABLE_Z],
            PaletteEntry::with_props("minecraft:oak_slab", &[("type", "bottom")]),
            None,
        ));
        for z in [TABLE_Z - 1, TABLE_Z + 1] {
            cells.push((
                [x, 1, z],
                PaletteEntry::with_props("minecraft:spruce_slab", &[("type", "bottom")]),
                None,
            ));
        }
    }
    let (slo, shi) = SIDE_TABLE;
    for x in slo[0]..=shi[0] {
        cells.push((
            [x, 1, slo[2]],
            PaletteEntry::simple("minecraft:oak_fence"),
            None,
        ));
        cells.push((
            [x, 2, slo[2]],
            PaletteEntry::with_props("minecraft:oak_slab", &[("type", "bottom")]),
            None,
        ));
        for z in [slo[2] - 1, slo[2] + 1] {
            cells.push((
                [x, 1, z],
                PaletteEntry::with_props("minecraft:spruce_slab", &[("type", "bottom")]),
                None,
            ));
        }
    }
    for light in [[2, 4, 2], [8, 4, 2], [2, 4, 10], [8, 4, 10]] {
        cells.push((light, PaletteEntry::simple("minecraft:glowstone"), None));
    }
    synth(SIZE, &cells)
}

/// The hall's document: hello-room's anchors, plus the table when `declared`.
fn hall_meta(s: &Structure, table: Table, declared: bool) -> serde_json::Value {
    let x = table.x_range();
    let mut anchors = serde_json::json!({
        "spawn": { "pos": [5, 1, 2], "facing": "south", "role": "entry" },
        "anchor/keeper-stand": { "pos": [5, 1, 4], "facing": "north" },
        "anchor/door": {
            "region": { "from": [4, 1, 6], "to": [5, 3, 6] },
            "block": "minecraft:iron_bars"
        },
        "anchor/exit": { "pos": EXIT },
        "anchor/table-top": { "pos": TABLE_TOP_CELL, "facing": "south" },
        "anchor/porter": { "pos": [3, 1, 4], "facing": "south" },
    });
    if declared {
        anchors["anchor/high-table"] = serde_json::json!({
            "role": "furniture",
            "region": { "from": [*x.start(), 1, TABLE_Z], "to": [*x.end(), 2, TABLE_Z] },
        });
        anchors["anchor/side-table"] = serde_json::json!({
            "role": "furniture",
            "region": { "from": SIDE_TABLE.0, "to": SIDE_TABLE.1 },
        });
    }
    serde_json::json!({
        "prefab_id": "prefab/hello-room",
        "structure": {
            "file": "hello-room.nbt",
            "id": "hello-room",
            "size": s.size,
            "data_version": s.data_version,
        },
        "anchors": anchors,
        "connectors": [],
        "walk_y": 1,
        "license": {
            "source": "original",
            "spdx": "GPL-3.0-or-later",
            "note": "Test fixture.",
            "provenance": "Synthesised by crates/delvec/tests/furniture.rs.",
        },
    })
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("furniture-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A one-piece prefab library holding the hall.
fn library(name: &str, table: Table, declared: bool) -> PathBuf {
    let dir = scratch(name);
    let s = hall(table);
    std::fs::write(dir.join("hello-room.nbt"), s.write()).unwrap();
    std::fs::write(
        dir.join("hello-room.json"),
        serde_json::to_string_pretty(&hall_meta(&s, table, declared)).unwrap(),
    )
    .unwrap();
    dir
}

fn hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// hello-world, with `npcs`/`quests` rewritten by the caller's edits.
fn campaign(
    edit_npcs: impl FnOnce(&mut serde_json::Value),
    edit_quests: impl FnOnce(&mut serde_json::Value),
) -> Campaign {
    let mut npcs: serde_json::Value = serde_json::from_str(&hw("npcs.json")).unwrap();
    edit_npcs(&mut npcs);
    let mut quests: serde_json::Value = serde_json::from_str(&hw("quests.json")).unwrap();
    edit_quests(&mut quests);
    let raw = RawCampaign {
        world: hw("world.json"),
        npcs: npcs.to_string(),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    let mut c = parse_campaign(&raw).expect("campaign parses");
    delvewright_dsl::tag_translatables(&mut c);
    c
}

fn plain() -> Campaign {
    campaign(|_| {}, |_| {})
}

fn structures(dir: &Path, plan: &Plan) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                out.insert(
                    t.structure_file.clone(),
                    std::fs::read(dir.join(&t.structure_file)).unwrap(),
                );
            }
        }
    }
    out
}

fn try_build(dir: &Path, c: &Campaign) -> Result<BuildOutput, emit::BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(dir).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let s = structures(dir, &plan);
    emit::build(
        &plan,
        &BTreeMap::new(),
        &s,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| panic!("`{path}` is emitted"))
            .clone(),
    )
    .unwrap()
}

fn failure(r: Result<BuildOutput, emit::BuildFailure>) -> (String, String) {
    match r {
        Ok(_) => panic!("expected the build to be refused"),
        Err(emit::BuildFailure::Diagnostic { code, message }) => (code.to_string(), message),
        Err(other) => panic!("expected a diagnostic, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// §5 — the byte claim, one shape per test
// ---------------------------------------------------------------------------

fn judge(meta: &PrefabMeta, s: &Structure) -> claims::ClaimVerdict {
    let grid = delvec::admit::spatial::grid(s);
    let facts = delvec::admit::settling::ByteFacts::of(&[([0, 0, 0], s)]);
    claims::check_piece(meta, &grid, &facts)
}

fn meta_of(v: serde_json::Value) -> PrefabMeta {
    serde_json::from_value(v).expect("the hall's document parses")
}

fn furniture_denials(v: &claims::ClaimVerdict) -> Vec<&claims::Claim> {
    v.denied
        .iter()
        .filter(|c| c.key == ClaimKey::AnchorFurniture)
        .collect()
}

/// The honest declaration is examined, not denied, and the census names the key.
#[test]
fn an_honest_furniture_declaration_is_examined_and_borne_out() {
    let s = hall(Table::Gap);
    let v = judge(&meta_of(hall_meta(&s, Table::Gap, true)), &s);
    assert!(furniture_denials(&v).is_empty(), "{:?}", v.denied);
    assert_eq!(ClaimKey::ALL.len(), 10, "the class holds ten keys");
    assert_eq!(
        v.binding.per_key.get("anchors.*.furniture"),
        Some(&(2, 0)),
        "{}",
        v.binding.line()
    );
    assert!(
        v.binding.line().contains("anchors.*.furniture=2/0"),
        "{}",
        v.binding.line()
    );
}

/// Shape 1: the role with no region is a declaration with no subject.
#[test]
fn furniture_with_no_region_is_dw0888() {
    let s = hall(Table::Gap);
    let mut doc = hall_meta(&s, Table::Gap, true);
    doc["anchors"]["anchor/high-table"]
        .as_object_mut()
        .unwrap()
        .remove("region");
    doc["anchors"]["anchor/high-table"]["pos"] = serde_json::json!([3, 2, TABLE_Z]);
    let v = judge(&meta_of(doc), &s);
    let d = furniture_denials(&v);
    assert_eq!(d.len(), 1, "{:?}", v.denied);
    assert!(d[0].short.contains("no `region`"), "{}", d[0].short);
    assert_eq!(claims::DW_CLAIM_DENIED.to_string(), "DW0888");
}

/// Shape 2: a region holding no solid block is furniture drawn over air — here
/// one course above the table top.
#[test]
fn furniture_over_air_is_dw0888() {
    let s = hall(Table::Gap);
    let mut doc = hall_meta(&s, Table::Gap, true);
    doc["anchors"]["anchor/high-table"]["region"] =
        serde_json::json!({ "from": [1, 3, TABLE_Z], "to": [7, 3, TABLE_Z] });
    let v = judge(&meta_of(doc), &s);
    let d = furniture_denials(&v);
    assert_eq!(d.len(), 1, "{:?}", v.denied);
    assert!(
        d[0].short.contains("holds no solid block"),
        "{}",
        d[0].short
    );
}

/// Shape 3: a region over solid blocks no body can stand on withholds nothing —
/// the table's fence legs alone, under their own top.
#[test]
fn furniture_that_withholds_nothing_is_dw0888() {
    let s = hall(Table::Gap);
    let mut doc = hall_meta(&s, Table::Gap, true);
    doc["anchors"]["anchor/high-table"]["region"] =
        serde_json::json!({ "from": [1, 1, TABLE_Z], "to": [7, 1, TABLE_Z] });
    let v = judge(&meta_of(doc), &s);
    let d = furniture_denials(&v);
    assert_eq!(d.len(), 1, "{:?}", v.denied);
    assert!(d[0].short.contains("stands a body on it"), "{}", d[0].short);
}

/// §4.2 / §9.7: the admission's standable rule is unchanged. Asked twice of one
/// grid, once beside a document without the declaration and once beside one
/// with it, the set is the same set — and it still holds the table top, which is
/// the fact the claim above withholds from walking and the admission does not.
#[test]
fn the_admission_standable_set_does_not_read_the_role() {
    let s = hall(Table::Gap);
    let grid = delvec::admit::spatial::grid(&s);
    let without = meta_of(hall_meta(&s, Table::Gap, false));
    let with = meta_of(hall_meta(&s, Table::Gap, true));
    assert!(!without.anchors.contains_key("anchor/high-table"));
    assert_eq!(
        with.anchors["anchor/high-table"].role,
        Some(AnchorRole::Furniture)
    );
    let before = delvec::schem::nav::standable_cells(&grid);
    let after = delvec::schem::nav::standable_cells(&grid);
    assert_eq!(before, after);
    assert!(
        before.contains(&TABLE_TOP_CELL),
        "the admission reads the table top as a place a body can stand"
    );
    // …and the table's blocks DO change the set: the same hall with no table.
    let bare = synth(
        SIZE,
        &hall(Table::Gap)
            .blocks
            .iter()
            .filter_map(|b| {
                let e = &hall(Table::Gap).palette[b.state as usize];
                let furniture = e.name.ends_with("_fence") || e.name.ends_with("_slab");
                (!furniture && e.name != "minecraft:air").then(|| (b.pos, e.clone(), None))
            })
            .collect::<Vec<_>>(),
    );
    assert_ne!(
        delvec::schem::nav::standable_cells(&delvec::admit::spatial::grid(&bare)),
        before
    );
}

// ---------------------------------------------------------------------------
// §4 — what the walk model does with it
// ---------------------------------------------------------------------------

/// §9.5: the only route crosses the table → `DW0510`'s furniture shape, naming
/// the anchor and no lethal volume. Without the declaration the same room builds.
#[test]
fn a_route_that_exists_only_over_furniture_is_dw0510() {
    let declared = library("wall-declared", Table::Wall, true);
    let (code, message) = failure(try_build(&declared, &plain()));
    assert_eq!(code, "DW0510", "{message}");
    assert!(message.contains("`anchor/high-table`"), "{message}");
    assert!(message.contains("OVER furniture"), "{message}");
    assert!(!message.contains("lethal volume"), "{message}");

    let undeclared = library("wall-undeclared", Table::Wall, false);
    let out = try_build(&undeclared, &plain()).expect("without the declaration it routes");
    let gate: serde_json::Value =
        serde_json::from_str(&text(&out, "validation/furniture-gate.json")).unwrap();
    assert_eq!(gate["examined"], 0, "{gate}");
    assert_eq!(gate["withheld"], 0, "{gate}");
}

/// §9.5 and §9.8: with a way round, the room builds; no exported waypoint rests
/// on the table; the ledger states a positive binding.
#[test]
fn a_route_with_a_way_round_builds_and_stands_no_waypoint_on_furniture() {
    let dir = library("gap-declared", Table::Gap, true);
    let out = try_build(&dir, &plain()).expect("the way round routes");
    let gate: serde_json::Value =
        serde_json::from_str(&text(&out, "validation/furniture-gate.json")).unwrap();
    assert_eq!(gate["examined"], 2, "{gate}");
    assert_eq!(gate["on_furniture"], 0, "{gate}");
    assert!(gate["withheld"].as_u64().unwrap() >= 1, "{gate}");
    assert_eq!(
        gate["anchors"],
        serde_json::json!(["anchor/high-table", "anchor/side-table"])
    );

    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let campaign = plain();
    let plan = Plan::build(&campaign, &prefabs).unwrap();
    assert_eq!(plan.furniture.len(), 2);
    let on_furniture = |support: [i32; 3]| {
        plan.furniture
            .iter()
            .any(|(_, (lo, hi))| (0..3).all(|i| lo[i] <= support[i] && support[i] <= hi[i]))
    };
    let wp: serde_json::Value =
        serde_json::from_str(&text(&out, "validation/critical-path-waypoints.json")).unwrap();
    let mut examined = 0;
    for leg in wp["legs"].as_array().unwrap() {
        for p in leg["waypoints"].as_array().unwrap() {
            let c: Vec<i64> = p
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect();
            let support = [c[0] as i32, c[1] as i32 - 1, c[2] as i32];
            examined += 1;
            assert!(!on_furniture(support), "waypoint {c:?} rests on a table");
        }
    }
    assert!(examined > 0, "the waypoint export bound to nothing");
}

/// §9.4: a body is POSTED on furniture where it is declared, and never WALKED
/// onto it. The keeper's stand moves to the table top: the summon carries that
/// cell and nothing names the table. A `move-npc` to the table top ends on the
/// floor its snap finds.
#[test]
fn a_body_is_posted_on_furniture_and_never_walked_onto_it() {
    let dir = library("post", Table::Gap, true);
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();

    let posted = campaign(
        |npcs| npcs["content"]["npcs"][0]["anchor"] = serde_json::json!("anchor/table-top"),
        |quests| {
            quests["content"]["quests"][0]["cast"]["npc/keeper"]["at"] =
                serde_json::json!("anchor/table-top");
            quests["content"]["quests"][0]["cast"]["npc/keeper"]["doing"] =
                serde_json::json!("keeping to anchor/table-top");
        },
    );
    let plan = Plan::build(&posted, &prefabs).unwrap();
    let delvec::compiler::plan::ResolvedAnchor::Point { pos, .. } = plan
        .anchors
        .get(&("area/keep".to_string(), "anchor/table-top".to_string()))
        .expect("the table-top anchor resolves")
    else {
        panic!("a point anchor");
    };
    let pos = *pos;
    let s = structures(&dir, &plan);
    let (out, warnings) = match emit::build_with_warnings(
        &plan,
        &BTreeMap::new(),
        &s,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    ) {
        Ok(v) => v,
        Err(e) => panic!("a body posted on a table builds: {e:?}"),
    };
    let named: Vec<&str> = warnings
        .iter()
        .map(|d| d.message.as_str())
        .filter(|m| m.contains("anchor/side-table") || m.contains("anchor/table-top"))
        .collect();
    assert!(
        named.is_empty(),
        "no diagnostic names the table: {named:#?}"
    );
    // The summon stands the body at the cell's centre on its floor, which on a
    // table is the slab's top: x and z are the cell's, y is at or above its
    // floor and below its head.
    let summons: Vec<String> = out
        .iter()
        .filter(|(path, _)| path.ends_with(".mcfunction"))
        .flat_map(|(_, bytes)| {
            String::from_utf8_lossy(bytes)
                .lines()
                .filter(|l| l.contains("summon minecraft:villager"))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect();
    let at_the_table = summons.iter().any(|l| {
        let xyz: Vec<f64> = l
            .split_whitespace()
            .skip_while(|w| !w.starts_with("minecraft:villager"))
            .skip(1)
            .take(3)
            .filter_map(|w| w.parse().ok())
            .collect();
        xyz.len() == 3
            && xyz[0] == f64::from(pos[0]) + 0.5
            && xyz[2] == f64::from(pos[2]) + 0.5
            && xyz[1] >= f64::from(pos[1]) - 1.0
            && xyz[1] < f64::from(pos[1]) + 1.0
    });
    assert!(
        at_the_table,
        "the keeper is summoned on the table top {pos:?}: {summons:#?}"
    );

    let walked = campaign(
        |_| {},
        |quests| {
            quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "type": "move-npc",
                    "npc": "npc/keeper",
                    "to": { "anchor": "anchor/table-top" }
                }));
        },
    );
    let plan = Plan::build(&walked, &prefabs).unwrap();
    let world = delvec::compiler::nav::World::from_plan(&plan, &structures(&dir, &plan));
    let moves = delvec::compiler::nav::plan_moves(&plan, &world).expect("the walk routes");
    let leg = moves
        .iter()
        .find(|m| m.to.anchor.as_str() == "anchor/table-top")
        .expect("the move is planned");
    assert_ne!(leg.target, pos, "the leg does not end on the table top");
    let (_, (lo, hi)) = plan
        .furniture
        .iter()
        .find(|(name, _)| name == "anchor/side-table")
        .expect("the side table is placed");
    let (lo, hi) = (*lo, *hi);
    let support = [leg.target[0], leg.target[1] - 1, leg.target[2]];
    assert!(
        !(0..3).all(|i| lo[i] <= support[i] && support[i] <= hi[i]),
        "the snapped cell {:?} does not rest on the table",
        leg.target
    );
    assert!(
        leg.cells.iter().all(|c| {
            let s = [c[0], c[1] - 1, c[2]];
            !(0..3).all(|i| lo[i] <= s[i] && s[i] <= hi[i])
        }),
        "no cell of the walk rests on the table: {:?}",
        leg.cells
    );
    try_build(&dir, &walked).expect("the walk to the table's side builds");
}

/// §4.3: a walked `move-npc` or `move-actor` leg whose only way is over the
/// table is `DW0510`'s furniture shape too, naming the anchor — not `DW0307` /
/// `DW0325`, which would send the author to look for a wall that is not there.
#[test]
fn a_walked_leg_that_exists_only_over_furniture_is_dw0510() {
    let dir = library("walked-wall", Table::Wall, true);
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let walked = campaign(
        |_| {},
        |quests| {
            quests["content"]["actors"] = serde_json::json!([{
                "id": "actor/porter",
                "entity": "minecraft:villager",
                "name": "The Porter",
                "anchor": "anchor/porter"
            }]);
            let talk = quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
                .as_array_mut()
                .unwrap();
            talk.push(serde_json::json!({
                "type": "move-npc", "npc": "npc/keeper", "to": { "anchor": "anchor/exit" }
            }));
            talk.push(serde_json::json!({ "type": "spawn-actor", "actor": "actor/porter" }));
            talk.push(serde_json::json!({
                "type": "move-actor", "actor": "actor/porter", "to": { "anchor": "anchor/exit" }
            }));
        },
    );
    let plan = Plan::build(&walked, &prefabs).unwrap();
    let world = delvec::compiler::nav::World::from_plan(&plan, &structures(&dir, &plan));
    for (verb, result) in [
        (
            "move-npc",
            delvec::compiler::nav::plan_moves(&plan, &world).map(|_| ()),
        ),
        (
            "move-actor",
            delvec::compiler::nav::plan_actor_moves(&plan, &world).map(|_| ()),
        ),
    ] {
        let f = result.expect_err("the only way is over the table");
        assert_eq!(f.code.to_string(), "DW0510", "{verb}: {}", f.message);
        assert!(f.message.starts_with(verb), "{verb}: {}", f.message);
        assert!(
            f.message.contains("`anchor/high-table`"),
            "{verb}: {}",
            f.message
        );
    }
    // The same walks over the same room with the furniture undeclared route.
    let bare = library("walked-wall-bare", Table::Wall, false);
    let prefabs = PrefabRegistry::load_dir(&bare).unwrap();
    let plan = Plan::build(&walked, &prefabs).unwrap();
    let world = delvec::compiler::nav::World::from_plan(&plan, &structures(&bare, &plan));
    delvec::compiler::nav::plan_moves(&plan, &world).expect("move-npc routes over it");
    delvec::compiler::nav::plan_actor_moves(&plan, &world).expect("move-actor routes over it");
}

/// §10.10: regions enter the plan in placed-piece order and anchor-name order,
/// and the same inputs give the same list and the same bytes.
#[test]
fn furniture_enters_the_plan_in_a_fixed_order_and_builds_byte_identically() {
    let dir = library("order", Table::Gap, true);
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let c = plain();
    let a = Plan::build(&c, &prefabs).unwrap().furniture;
    let b = Plan::build(&c, &prefabs).unwrap().furniture;
    assert_eq!(a, b);
    let names: Vec<&str> = a.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["anchor/high-table", "anchor/side-table"]);
    let first = try_build(&dir, &plain()).unwrap();
    let second = try_build(&dir, &plain()).unwrap();
    assert_eq!(first, second);
}
