//! **A campaign opening a piece's contingent way** (spec-0042 §2.4/§2.5, DSL
//! v0.12): what `open-way` emits, what the completability proof concludes from
//! it, and what the disposition enumeration binds to.
//!
//! # The artifacts are real
//!
//! The way-carrying piece is exported by `crates/grammar` from a program built
//! here — the corpus contract piece with its doorway's threshold course claimed
//! as `deck` and left empty, and the door declared a `walk` whose way is `laid`.
//! As built the two rooms are severed: a body cannot stand on a threshold that
//! is not there. The export runs the prefab checker's contract gates, so a piece
//! that reaches these tests has already proved, on its own bytes, that the break
//! is real and that laying the deck joins it.
//!
//! That is what makes the campaign-side question the only open one: **does
//! anything lay it, and in time?**

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};
use delvewright_grammar::ir::{
    Contract, EXTERIOR, EdgeClass, Envelope, Mark, MarkAt, Node, Opens, Program, Reorient,
    Rounding, Size, Split, Way,
};
use delvewright_grammar::{Axis, BlockState, Box3, ExpandOptions, export_prefab};

/// The vault is `9 x 8 x 11`: a low room, a raised room, a flight between them
/// whose treads are missing, and a shaft through the roof.
const PIECE: Box3 = Box3::at_origin([9, 8, 11]);

/// The prefab id every fixture here binds.
const TOWER: &str = "prefab/broken-threshold";

// ---------------------------------------------------------------------------
// The piece
// ---------------------------------------------------------------------------

/// **The broken flight**: a sealed vault, entered by a shaft through its roof,
/// whose one internal climb is severed as built and joined by a `laid` way.
///
/// ```text
///          shaft (exterior --drop--> near)
///            │
///   ┌────────┼────────────────────────────┐  y7  roof
///   │        ▼                            │  y6  ┐
///   │                          ┌──────────┤  y5  ├ upper band
///   │      near         ·      │   far    │  y4  ┘   (far stands here)
///   │                ·  ·  ·   ├──────────┤  y3  ┐
///   │            ·             │//////////│  y2  ├ lower band
///   │  ·  ·  ·  ·              │//////////│  y1  ┘   (near stands here)
///   └───────────────────────────..........┘  y0  ground
///           near   flight (·= a missing tread)
/// ```
///
/// **Every part of that shape is load-bearing, and none of it is decoration.**
///
/// *Sealed, and entered from above.* A contract's entry space must declare a way
/// in from outside, and a piece placed alone over the void has void outside: a
/// horizontal opening is a walkable cell with nothing beyond it, which the
/// boundary proof refuses (`DW0322`) and should. A shaft through the roof is the
/// same claim made vertically — a body falls in, and no reachable cell of the
/// vault ever stands beside a bottomless column.
///
/// *The break is a climb, not a hole.* Missing treads leave the ground under
/// them, so the flight's floor is ordinary reachable standing room and the far
/// room is three blocks up a wall. A missing floor would strand a body in a pit
/// instead — standable cells the walk cannot reach, which the piece's own
/// reachability gate refuses, and rightly.
///
/// `block` is what the treads' palette role binds, so two fixtures can differ in
/// **what the metadata says the way is built from** and in nothing else — which
/// is how "the block comes from the piece" is told apart from "the compiler
/// writes a block it happens to know".
fn broken_flight(block: &str) -> Program {
    let split = |axis: Axis, sizes: Vec<Size>, children: Vec<Node>| {
        Node::Split(Split {
            axis,
            sizes,
            rounding: Rounding::Truncate,
            repeat: false,
            orient: Reorient::KEEP,
            children,
        })
    };
    let claim = |region: &str, body: Node| Node::Claim {
        region: region.to_string(),
        body: Box::new(body),
    };
    // One tread, at its own course of the flight: claimed as the way's region and
    // left EMPTY, which is what makes the flight a break on the bytes as shipped.
    let tread = || claim("broken-flight", Node::Void);
    let walled = |middle: Node| {
        split(
            Axis::X,
            vec![Size::abs(1), Size::abs(7), Size::abs(1)],
            vec![Node::fill("shell"), middle, Node::fill("shell")],
        )
    };
    let flanked = |middle: Node| {
        split(
            Axis::X,
            vec![Size::abs(3), Size::abs(3), Size::abs(3)],
            vec![Node::fill("shell"), middle, Node::fill("shell")],
        )
    };
    Program::new("broken-flight", "vault")
        .role("shell", BlockState::simple("stone_bricks"))
        .role("tread", BlockState::simple(block))
        .contract(
            Contract::new("near")
                .space("near", Envelope::Enclosed)
                .space("far", Envelope::Enclosed)
                // The way in: a body drops through the roof shaft into the near
                // room. An `exterior` endpoint has no box on the far side to
                // measure a level against, so it declares no rise.
                .edge(
                    EXTERIOR,
                    "near",
                    EdgeClass::Drop {
                        rise: 0,
                        via: Some("shaft".to_string()),
                        way: None,
                    },
                )
                .edge(
                    "near",
                    "far",
                    EdgeClass::Stair {
                        rise: 3,
                        via: "flight".to_string(),
                        way: Some(Way {
                            opens: Opens::Laid,
                            region: "broken-flight".to_string(),
                            block: "tread".to_string(),
                        }),
                    },
                ),
        )
        .rule(
            "vault",
            split(
                Axis::Y,
                vec![Size::abs(1), Size::abs(3), Size::abs(3), Size::abs(1)],
                vec![
                    Node::fill("shell"),
                    Node::call("lower"),
                    Node::call("upper"),
                    Node::call("roof"),
                ],
            ),
        )
        // y1..y3: the near room, the flight, and the raised room's substructure.
        .rule(
            "lower",
            split(
                Axis::Z,
                vec![
                    Size::abs(1),
                    Size::abs(3),
                    Size::abs(3),
                    Size::abs(3),
                    Size::abs(1),
                ],
                vec![
                    Node::fill("shell"),
                    walled(claim("near", Node::Void)),
                    flanked(claim("flight", Node::call("treads"))),
                    Node::fill("shell"),
                    Node::fill("shell"),
                ],
            ),
        )
        // The flight itself: one tread per course, each one course further along.
        .rule(
            "treads",
            split(
                Axis::Z,
                vec![Size::abs(1), Size::abs(1), Size::abs(1)],
                vec![
                    split(
                        Axis::Y,
                        vec![Size::abs(1), Size::abs(2)],
                        vec![tread(), Node::Void],
                    ),
                    split(
                        Axis::Y,
                        vec![Size::abs(1), Size::abs(1), Size::abs(1)],
                        vec![Node::Void, tread(), Node::Void],
                    ),
                    split(
                        Axis::Y,
                        vec![Size::abs(2), Size::abs(1)],
                        vec![Node::Void, tread()],
                    ),
                ],
            ),
        )
        // y4..y6: the near room's headroom, the top of the flight, the far room.
        // The far room carries the anchor a campaign objective stands on — the
        // whole reason the far room is required content rather than a room.
        .rule(
            "upper",
            split(
                Axis::Z,
                vec![
                    Size::abs(1),
                    Size::abs(3),
                    Size::abs(3),
                    Size::abs(3),
                    Size::abs(1),
                ],
                vec![
                    Node::fill("shell"),
                    walled(claim("near", Node::Void)),
                    // The whole headroom over the flight belongs to the flight:
                    // a space's `enclosed` boundary is accounted for by a
                    // declared opening, and the air a climber's head passes
                    // through is part of the climb.
                    flanked(claim("flight", Node::Void)),
                    walled(claim(
                        "far",
                        Node::Mark {
                            mark: Mark::new("goal", MarkAt::FloorCenter),
                            body: Box::new(Node::Void),
                        },
                    )),
                    Node::fill("shell"),
                ],
            ),
        )
        // y7: the roof, with the shaft the vault is entered by.
        .rule(
            "roof",
            split(
                Axis::Z,
                vec![Size::abs(2), Size::abs(1), Size::abs(8)],
                vec![
                    Node::fill("shell"),
                    split(
                        Axis::X,
                        vec![Size::abs(4), Size::abs(1), Size::abs(4)],
                        vec![
                            Node::fill("shell"),
                            claim("shaft", Node::Void),
                            Node::fill("shell"),
                        ],
                    ),
                    Node::fill("shell"),
                ],
            ),
        )
}

/// Give the exported piece the bare `spawn` anchor an area needs to be
/// transported INTO.
///
/// Written into the metadata rather than declared in the program, and that is
/// not a shortcut: a grammar `mark` always exports as `anchor/<stem>`, so a
/// generated piece cannot name the un-namespaced `spawn` key the inter-area
/// transport looks up (`plan::build_critical_path`). Any campaign that puts a
/// generated zone in an area of its own meets this, and it is reported with this
/// step rather than worked around silently.
fn with_a_spawn(dir: &Path, id: &str) {
    let path = dir.join(format!("{id}.json"));
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    // The near room's floor, under the shaft a body falls through.
    meta["anchors"]["spawn"] = serde_json::json!({ "pos": [4, 1, 2], "facing": "south" });
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
}

/// Give the exported piece a second edge whose way runs the OTHER direction: a
/// `cleared` region, standing in the piece's own shell as built, that opening
/// voids.
///
/// Written into the metadata rather than into the program, and the split is the
/// honest one: whether a `cleared` way's blocks really sever what it claims is
/// the prefab checker's question, proved on the bytes at export. What the
/// compiler owes is that the SIGN decides what the effect writes, and a metadata
/// document is exactly what the compiler is handed (spec-0042 §1.5).
fn with_a_cleared_way(dir: &Path, id: &str) {
    let path = dir.join(format!("{id}.json"));
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    meta["spatial_contract"]["edges"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "a": "near",
            "b": "far",
            "class": "walk",
            "way": {
                "opens": "cleared",
                "region": "rubble",
                "boxes": [ { "from": [0, 1, 4], "to": [2, 1, 4] } ],
                "block": "minecraft:stone_bricks"
            }
        }));
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
}

/// The same piece with a **second** way region that resolves to no cells at all:
/// declared in the contract, present in the metadata, and staging nothing.
///
/// Written by hand into the exported metadata rather than by a program, because
/// the grammar refuses to build it — which is the point: what this exercises is
/// a metadata document reaching the compiler, whatever wrote it.
fn with_a_cell_less_way(dir: &Path, id: &str) {
    let path = dir.join(format!("{id}.json"));
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let edges = meta["spatial_contract"]["edges"].as_array_mut().unwrap();
    edges.push(serde_json::json!({
        "a": "near",
        "b": "far",
        "class": "walk",
        "way": {
            "opens": "laid",
            "region": "phantom",
            "boxes": [],
            "block": "minecraft:stone"
        }
    }));
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
}

/// A prefab library, the way it exports, and what that way is made of: the
/// directory, the way's LOCAL boxes and its block state, read back out of the
/// exported document so every expectation below is anchored on what the piece
/// says rather than on what this file believes.
type Library = (PathBuf, Vec<([i32; 3], [i32; 3])>, String);

/// A private prefab library: the real one, plus the way-carrying piece.
///
/// Returns the directory and the way's **local** boxes and block as the exported
/// metadata states them — read back out of the document, so every expectation
/// below is anchored on what the piece says rather than on what this file
/// believes.
fn library(tag: &str, deck_block: &str) -> Library {
    let dir = std::env::temp_dir().join(format!("dw-open-way-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let exported = export_prefab(
        &broken_flight(deck_block),
        PIECE,
        &ExpandOptions::seeded(1),
        "broken-threshold",
    )
    .expect("the broken threshold exports green with its way declared");
    exported.write_to_dir(&dir).unwrap();
    with_a_spawn(&dir, "broken-threshold");

    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("broken-threshold.json")).unwrap())
            .unwrap();
    let way = meta["spatial_contract"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|e| e.get("way"))
        .expect("the door carries a way");
    assert_eq!(way["opens"], "laid");
    assert_eq!(way["region"], "broken-flight");
    let boxes: Vec<([i32; 3], [i32; 3])> = way["boxes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| (cell(&b["from"]), cell(&b["to"])))
        .collect();
    assert!(!boxes.is_empty(), "binding count 0: the way has no cells");
    let block = way["block"].as_str().unwrap().to_string();
    (dir, boxes, block)
}

fn cell(v: &serde_json::Value) -> [i32; 3] {
    let a = v.as_array().unwrap();
    [
        a[0].as_i64().unwrap() as i32,
        a[1].as_i64().unwrap() as i32,
        a[2].as_i64().unwrap() as i32,
    ]
}

// ---------------------------------------------------------------------------
// The campaign
// ---------------------------------------------------------------------------

fn hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// Hello-world's world with a second area holding the way-carrying piece.
fn world_doc() -> String {
    r#"{
  "dsl_version": "0.2.0",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home. The Keeper holds the key, and only conversation will move him.",
    "seed": 20260729,
    "target_minutes": 5,
    "areas": [
      { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" },
      { "id": "area/tower", "name": "The Tower", "prefab": "prefab/broken-threshold",
        "mitigation": "night-vision" }
    ]
  }
}"#
    .to_string()
}

/// The plan. `tower` decides whether anything in the campaign points past the
/// break at all — which is the difference between required content behind a
/// way and scenery behind one.
fn quest_plan_doc(tower: bool) -> String {
    if !tower {
        return r#"{
  "dsl_version": "0.2.0",
  "campaign_id": "hello-world",
  "stage": "quest-plan",
  "content": {
    "finale": "quest/open-the-door",
    "quests": [
      { "id": "quest/open-the-door", "act": 1, "area": "area/keep", "depends_on": [],
        "goal": "Get the Keeper to open the door and leave the keep.",
        "mandatory": true, "npcs": ["npc/keeper"] }
    ]
  }
}"#
        .to_string();
    }
    r#"{
  "dsl_version": "0.2.0",
  "campaign_id": "hello-world",
  "stage": "quest-plan",
  "content": {
    "finale": "quest/cross",
    "quests": [
      { "id": "quest/open-the-door", "act": 1, "area": "area/keep", "depends_on": [],
        "goal": "Get the Keeper to open the door and leave the keep.",
        "mandatory": true, "npcs": ["npc/keeper"] },
      { "id": "quest/cross", "act": 1, "area": "area/tower",
        "depends_on": ["quest/open-the-door"],
        "goal": "Cross the mended threshold.", "mandatory": true, "npcs": [] }
    ]
  }
}"#
    .to_string()
}

/// The `open-way` effect, as a campaign writes it.
fn open_way(way: &str) -> serde_json::Value {
    serde_json::json!({ "type": "open-way", "piece": TOWER, "way": way })
}

/// Where the opening effect sits in the quest DAG — the axis AC9 turns on.
#[derive(Clone, Copy, PartialEq)]
enum Opening {
    /// On the first quest's first objective: forced, and a strict DAG ancestor
    /// of the objective beyond the break.
    Before,
    /// On the objective beyond the break itself: forced, and NOT before it.
    After,
    /// In the campaign's `on_death` bundle: it opens the way in play, and
    /// nobody is forced to die.
    Unforced,
    /// Nowhere. The deletion demonstration.
    None,
    /// On the first quest, naming a way no placed piece stages.
    Misnamed,
}

fn quests_doc(opening: Opening, tower: bool) -> String {
    let mut first: Vec<serde_json::Value> =
        vec![serde_json::json!({ "type": "open-gate", "anchor": "anchor/door" })];
    let mut cross: Vec<serde_json::Value> = Vec::new();
    let mut on_death: Vec<serde_json::Value> = Vec::new();
    match opening {
        Opening::Before => first.push(open_way("broken-flight")),
        Opening::After => cross.push(open_way("broken-flight")),
        Opening::Unforced => on_death.push(open_way("broken-flight")),
        Opening::Misnamed => first.push(open_way("no-such-way")),
        Opening::None => {}
    }
    let mut quests = vec![serde_json::json!({
      "id": "quest/open-the-door",
      "trigger": { "type": "campaign-start" },
      "objectives": [
        { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
        { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
          "radius": 2, "after": ["obj/talk"] }
      ],
      "on_objective_complete": { "obj/talk": first },
      "on_complete": if tower { serde_json::json!([]) }
                     else { serde_json::json!([{ "type": "campaign-complete" }]) }
    })];
    if tower {
        quests.push(serde_json::json!({
          "id": "quest/cross",
          "trigger": { "type": "campaign-start" },
          "objectives": [
            { "type": "reach-anchor", "id": "obj/goal", "anchor": "anchor/goal", "radius": 2 }
          ],
          "on_objective_complete": { "obj/goal": cross },
          "on_complete": [ { "type": "campaign-complete" } ]
        }));
    }
    let doc = serde_json::json!({
      "dsl_version": "0.12.0",
      "campaign_id": "hello-world",
      "stage": "quests",
      "content": { "on_death": on_death, "quests": quests }
    });
    serde_json::to_string_pretty(&doc).unwrap()
}

fn campaign(opening: Opening) -> Campaign {
    campaign_with(world_doc(), quests_doc(opening, true), true)
}

fn campaign_with(world: String, quests: String, tower: bool) -> Campaign {
    let raw = RawCampaign {
        world,
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: quest_plan_doc(tower),
        quests,
        dialogue: hw("dialogue.json"),
        world_edits: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

/// The refusal a campaign earns at plan time, or a panic naming what it built
/// instead. Returns the ERROR rather than the plan, because a `Plan` has no
/// `Debug` to unwrap against — and because every caller here is asserting a
/// refusal.
fn plan_err(c: &Campaign, dir: &Path, expected: &str) -> delvewright_compiler::plan::PlanError {
    let prefabs = PrefabRegistry::load_dir(dir).unwrap();
    match Plan::build(c, &prefabs) {
        Ok(_) => panic!("{expected}: the campaign built instead"),
        Err(e) => e,
    }
}

fn build(c: &Campaign, dir: &Path) -> BuildOutput {
    let prefabs = PrefabRegistry::load_dir(dir).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(dir.join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("the campaign builds")
}

/// Every `fill` line in every emitted function, in file order.
fn fill_lines(out: &BuildOutput) -> Vec<String> {
    let mut lines = Vec::new();
    for (path, bytes) in out {
        if !path.ends_with(".mcfunction") {
            continue;
        }
        for line in String::from_utf8_lossy(bytes).lines() {
            if line.trim_start().starts_with("fill ") {
                lines.push(line.trim().to_string());
            }
        }
    }
    lines
}

/// Where the tower piece was placed, so a local cell can be turned into the
/// world cell the emitted command must carry.
fn tower_origin(plan: &Plan) -> [i32; 3] {
    let area = plan
        .areas
        .iter()
        .find(|a| a.area_id == "area/tower")
        .expect("the tower area is placed");
    area.pieces[0].pos
}

// ---------------------------------------------------------------------------
// AC8 — the effect has one authority
// ---------------------------------------------------------------------------

/// **What an `open-way` emits is the piece's own metadata, cell for cell and
/// block for block** (spec-0042 AC8).
///
/// The expectation is not written down here: it is read out of the exported
/// prefab document and offset by where the piece was placed. A test that spelled
/// the coordinates itself would pass just as well against a compiler that spelled
/// them too.
#[test]
fn an_open_way_emits_exactly_the_exported_cells_with_the_exported_block() {
    let (dir, boxes, block) = library("emit", "oak_planks");
    let c = campaign(Opening::Before);
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let origin = tower_origin(&plan);
    let expected: Vec<String> = boxes
        .iter()
        .map(|(from, to)| {
            format!(
                "fill {} {} {} {} {} {} {block}",
                origin[0] + from[0],
                origin[1] + from[1],
                origin[2] + from[2],
                origin[0] + to[0],
                origin[1] + to[1],
                origin[2] + to[2],
            )
        })
        .collect();
    drop(plan);

    let out = build(&c, &dir);
    let fills = fill_lines(&out);
    assert!(
        !expected.is_empty(),
        "binding count 0: the way exported no boxes, so this asserted nothing"
    );
    for want in &expected {
        assert!(
            fills.iter().any(|l| l == want),
            "the way's fill is not what the piece says it is.\nwanted: {want}\nemitted: {fills:#?}"
        );
    }
    // …and exactly once per box: an `open-way` is not a fill repeated per
    // anything.
    for want in &expected {
        assert_eq!(fills.iter().filter(|l| *l == want).count(), 1, "{want}");
    }
}

/// **The block follows the piece, not the compiler.**
///
/// The same campaign JSON, byte for byte, against two pieces whose deck role
/// binds a different block. If the emitted fill were anything the compiler chose
/// — a constant, a default, a guess from the region's name — both builds would
/// write the same thing.
#[test]
fn two_pieces_with_different_deck_blocks_emit_different_fills() {
    let (stone_dir, _, stone_block) = library("block-a", "stone");
    let (plank_dir, _, plank_block) = library("block-b", "oak_planks");
    assert_ne!(
        stone_block, plank_block,
        "binding count 0: the two fixtures declare the same block, so this compares nothing"
    );
    let c = campaign(Opening::Before);
    let stone = fill_lines(&build(&c, &stone_dir));
    let plank = fill_lines(&build(&c, &plank_dir));
    assert!(
        stone.iter().any(|l| l.ends_with(&stone_block)),
        "{stone:#?}"
    );
    assert!(
        plank.iter().any(|l| l.ends_with(&plank_block)),
        "{plank:#?}"
    );
    assert!(
        !stone.iter().any(|l| l.ends_with(&plank_block)),
        "the stone-decked piece emitted the plank block: {stone:#?}"
    );
}

/// **There is no second authority to disagree with the first.**
///
/// A region, a block or a sign on the effect is not a field this document form
/// has, so the campaign cannot state one — which is why nothing anywhere
/// compares the effect's geometry against the piece's. The refusal is serde's,
/// and that is exactly right for a field that does not exist.
#[test]
fn an_open_way_carrying_a_region_a_block_or_a_sign_is_not_a_document_this_engine_reads() {
    for extra in [
        r#""region": { "anchor": "anchor/goal", "extent": [1, 1, 1] }"#,
        r#""block": "minecraft:stone""#,
        r#""opens": "laid""#,
        r#""boxes": []"#,
    ] {
        let json =
            format!(r#"{{ "type": "open-way", "piece": "prefab/x", "way": "deck", {extra} }}"#);
        let err = serde_json::from_str::<delvewright_dsl::QuestEffect>(&json)
            .expect_err("a second authority must be refused, not dropped");
        assert!(err.to_string().contains("unknown field"), "{extra}: {err}");
    }
}

/// **The other sign, on the same machinery**: a `cleared` way's opening writes
/// air, and it writes it over the cells the metadata gives it.
///
/// One verb, two directions, and the campaign says neither — which is the whole
/// point of reading the sign off the piece. An engine that emitted the way's
/// block here would wall the passage it was told to open.
#[test]
fn a_cleared_way_is_opened_by_writing_air_over_its_own_cells() {
    let (dir, _, block) = library("cleared", "oak_planks");
    with_a_cleared_way(&dir, "broken-threshold");
    let mut quests: serde_json::Value =
        serde_json::from_str(&quests_doc(Opening::Before, true)).unwrap();
    quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
        .as_array_mut()
        .unwrap()
        .push(open_way("rubble"));
    let c = campaign_with(
        world_doc(),
        serde_json::to_string_pretty(&quests).unwrap(),
        true,
    );
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let origin = tower_origin(&plan);
    let gate = plan.way_gate.as_ref().expect("two ways are staged");
    assert_eq!(gate.staged, 2);
    assert_eq!(gate.opened, 2);
    drop(plan);

    let fills = fill_lines(&build(&c, &dir));
    let cleared = format!(
        "fill {} {} {} {} {} {} minecraft:air",
        origin[0],
        origin[1] + 1,
        origin[2] + 4,
        origin[0] + 2,
        origin[1] + 1,
        origin[2] + 4,
    );
    assert!(
        fills.contains(&cleared),
        "the cleared way did not write air over its own cells.\nwanted: {cleared}\nemitted: {fills:#?}"
    );
    // …and the laid way on the same piece still writes its own block, so the
    // sign is read per way rather than per campaign.
    assert!(fills.iter().any(|l| l.ends_with(&block)), "{fills:#?}");
}

// ---------------------------------------------------------------------------
// AC9 — ordering has teeth
// ---------------------------------------------------------------------------

/// **The green**: the objective stands beyond the break, and a forced `open-way`
/// on an earlier quest lays the deck before the party has to cross.
#[test]
fn an_objective_beyond_a_laid_way_compiles_when_a_forced_opening_precedes_it() {
    let (dir, _, _) = library("ac9-green", "oak_planks");
    let out = build(&campaign(Opening::Before), &dir);
    let gate: serde_json::Value =
        serde_json::from_slice(&out["validation/ways.json"]).expect("the ways ledger is emitted");
    assert_eq!(gate["staged"], 1);
    assert_eq!(gate["opened"], 1);
    assert_eq!(gate["never_opened"], 0);
    assert!(
        gate["elements_examined"].as_u64().unwrap() > 0,
        "binding count 0: no required element was judged against a way, so the green says nothing"
    );
}

/// **The deletion demonstration** (spec-0042 AC9): take the opening effect away
/// and the same campaign is refused, naming the way and the objective behind it.
///
/// This is what makes every assertion above non-vacuous. If the far room were
/// reachable by any other route, the campaign would build with no opening at all
/// and every ordering claim about this way would be inert.
#[test]
fn deleting_the_opening_effect_reds_naming_the_way_and_the_objective() {
    let (dir, _, _) = library("ac9-deleted", "oak_planks");
    let c = campaign(Opening::None);
    let err = plan_err(
        &c,
        &dir,
        "an objective behind a way nothing opens is unwinnable",
    );
    assert_eq!(err.code.id(), "DW0548");
    assert!(err.message.contains("broken-flight"), "{}", err.message);
    assert!(err.message.contains("obj/goal"), "{}", err.message);
    assert!(err.message.contains("never opened"), "{}", err.message);
    // The room behind it, counted: a verdict that cannot say how much building
    // is stranded is a verdict nobody can size.
    assert!(err.message.contains("cell(s)"), "{}", err.message);
}

/// **The same campaign with the effect moved after the objective is red**, and
/// the refusal names the way, the effect and the objective.
#[test]
fn an_opening_that_does_not_precede_the_objective_reds_naming_all_three() {
    let (dir, _, _) = library("ac9-late", "oak_planks");
    let c = campaign(Opening::After);
    let err = plan_err(
        &c,
        &dir,
        "a deck laid on arrival is a deck the party fell past",
    );
    assert_eq!(err.code.id(), "DW0548");
    assert!(err.message.contains("broken-flight"), "{}", err.message);
    assert!(err.message.contains("obj/goal"), "{}", err.message);
    assert!(err.message.contains("open-way"), "{}", err.message);
    assert!(
        err.message.contains("does not put before"),
        "{}",
        err.message
    );
}

/// **The same campaign with the effect on an unforced root is red.**
///
/// An `open-way` in the `on_death` bundle really does lay the deck in play — for
/// a party that dies. Nobody is forced to die, so the opening proves nothing
/// about a route that needs it, and the refusal says which beat it was.
#[test]
fn an_opening_on_an_unforced_root_reds_naming_the_beat() {
    let (dir, _, _) = library("ac9-unforced", "oak_planks");
    let c = campaign(Opening::Unforced);
    let err = plan_err(
        &c,
        &dir,
        "a deck laid only on death is a deck nobody must lay",
    );
    assert_eq!(err.code.id(), "DW0548");
    assert!(err.message.contains("broken-flight"), "{}", err.message);
    assert!(err.message.contains("obj/goal"), "{}", err.message);
    assert!(
        err.message.contains("not forced to play"),
        "{}",
        err.message
    );
}

// ---------------------------------------------------------------------------
// AC11 — disposition binds
// ---------------------------------------------------------------------------

/// **The disposition is enumerated per staged way**, with the effect that opens
/// it and the quest-DAG point it fires at.
#[test]
fn the_ledger_names_the_effect_that_opens_each_way() {
    let (dir, boxes, block) = library("ledger", "oak_planks");
    let out = build(&campaign(Opening::Before), &dir);
    let gate: serde_json::Value = serde_json::from_slice(&out["validation/ways.json"]).unwrap();
    assert_eq!(gate["pieces_with_ways"], 1);
    assert_eq!(gate["open_way_effects"], 1);
    let row = &gate["ways"][0];
    assert_eq!(row["piece"], TOWER);
    assert_eq!(row["way"], "broken-flight");
    assert_eq!(row["opens"], "laid");
    assert_eq!(row["block"], block.as_str());
    let cells: u64 = boxes
        .iter()
        .map(|(f, t)| {
            (0..3)
                .map(|a| (t[a] - f[a]).unsigned_abs() as u64 + 1)
                .product::<u64>()
        })
        .sum();
    assert_eq!(row["cells"], cells);
    let opened = &row["opened_by"][0];
    assert_eq!(opened["forced"], true);
    assert_eq!(opened["stage"], "quests");
    assert!(
        opened["path"].as_str().unwrap().contains("obj/talk"),
        "{opened}"
    );
}

/// **A door that never opens is content** (spec-0042 §2.5): a way nothing opens
/// is enumerated as never-opened, with the cells behind it, and is not a
/// finding — so long as nothing required stands behind it.
#[test]
fn a_way_nobody_opens_is_reported_and_is_not_a_finding() {
    let (dir, _, _) = library("scenery", "oak_planks");
    // The objective beyond the break is what makes the far room REQUIRED. Take
    // the whole quest away and the same never-opened way is scenery: a door in
    // a building, opening on a room nothing asks the party to enter.
    let c = campaign_with(world_doc(), quests_doc(Opening::None, false), false);
    let out = build(&c, &dir);
    let gate: serde_json::Value = serde_json::from_slice(&out["validation/ways.json"]).unwrap();
    assert_eq!(gate["staged"], 1);
    assert_eq!(gate["never_opened"], 1);
    assert_eq!(gate["opened"], 0);
    assert!(gate["ways"][0]["cells"].as_u64().unwrap() > 0);
}

/// **A way with no cells is a way the enumeration never saw** (spec-0042 AC11):
/// declared in the metadata, staging nothing, and refused with the two counts
/// that disagree.
#[test]
fn a_declared_way_that_stages_no_cells_is_dw0549() {
    let (dir, _, _) = library("unstaged", "oak_planks");
    with_a_cell_less_way(&dir, "broken-threshold");
    let c = campaign(Opening::Before);
    let err = plan_err(&c, &dir, "a way with no cells opens nothing");
    assert_eq!(err.code.id(), "DW0549");
    assert!(err.message.contains("2 contingent way"), "{}", err.message);
    assert!(err.message.contains("only 1"), "{}", err.message);
}

// ---------------------------------------------------------------------------
// DW0547 — the reference names exactly one placed way
// ---------------------------------------------------------------------------

/// An `open-way` naming a way no placed piece stages is refused, and the refusal
/// prints what the world does stage.
#[test]
fn an_open_way_naming_no_staged_way_is_dw0547() {
    let (dir, _, _) = library("misnamed", "oak_planks");
    let c = campaign(Opening::Misnamed);
    let err = plan_err(&c, &dir, "a way that is not there cannot be opened");
    assert_eq!(err.code.id(), "DW0547");
    assert!(err.message.contains("no-such-way"), "{}", err.message);
    assert!(err.message.contains("broken-flight"), "{}", err.message);
}

/// **The reference is checked even when the world stages no way at all** — the
/// case a guard on the staged ways alone would skip in silence.
///
/// An `open-way` against the stock prefab library, where no placed piece declares
/// a contract of any kind: the effect would emit nothing, the disposition
/// enumeration would have nothing to enumerate, and the campaign would ship a
/// beat that does nothing. It is refused instead, and the refusal says the world
/// stages none.
#[test]
fn an_open_way_in_a_world_that_stages_no_way_is_still_dw0547() {
    let dir = common::prefabs_dir();
    let plain = r#"{
  "dsl_version": "0.2.0",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home. The Keeper holds the key, and only conversation will move him.",
    "seed": 20260729,
    "target_minutes": 5,
    "areas": [ { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" } ]
  }
}"#;
    let c = campaign_with(plain.to_string(), quests_doc(Opening::Before, false), false);
    let err = plan_err(&c, &dir, "a way no piece stages cannot be opened");
    assert_eq!(err.code.id(), "DW0547");
    assert!(
        err.message
            .contains("no placed piece stages any way at all"),
        "{}",
        err.message
    );
}

/// A piece placed twice stages two ways, and one reference names neither of them
/// rather than both.
#[test]
fn a_piece_placed_twice_makes_its_way_reference_ambiguous() {
    let (dir, _, _) = library("twice", "oak_planks");
    let mut world: serde_json::Value = serde_json::from_str(&world_doc()).unwrap();
    let areas = world["content"]["areas"].as_array_mut().unwrap();
    let mut twin = areas[1].clone();
    twin["id"] = serde_json::json!("area/tower-b");
    twin["name"] = serde_json::json!("The Other Tower");
    areas.push(twin);
    let world = serde_json::to_string_pretty(&world).unwrap();
    let c = campaign_with(world, quests_doc(Opening::Before, true), true);
    let err = plan_err(&c, &dir, "two placements, one reference");
    assert_eq!(err.code.id(), "DW0547");
    assert!(err.message.contains("2 placed pieces"), "{}", err.message);
    assert!(err.message.contains("area/tower-b"), "{}", err.message);
}

// ---------------------------------------------------------------------------
// DW0555 — the reachability half's binding count
// ---------------------------------------------------------------------------

/// Ways staged, nothing required behind any of them: the enumeration still
/// reports every way, and says out loud that its reachability half examined
/// nothing.
#[test]
fn ways_with_no_required_element_behind_them_raise_dw0555() {
    let (dir, _, _) = library("unbound", "oak_planks");
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let c = campaign_with(world_doc(), quests_doc(Opening::Before, false), false);
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let gate = plan.way_gate.as_ref().expect("a staged way has a ledger");
    assert_eq!(gate.staged, 1);
    assert_eq!(gate.elements_examined, 0);
    assert!(
        plan.warnings.iter().any(|d| d.code == "DW0555"),
        "{:#?}",
        plan.warnings
    );
}
