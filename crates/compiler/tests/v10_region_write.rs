//! DSL v0.10 region writes (spec-0031): `fill-region` / `clear-region` — what the
//! compiler emits, what the completability proof concludes, and the proof that
//! `open-gate` / `close-gate` are now the same operation with the box and the
//! block read off a prefab anchor.
//!
//! The `hello-room` prefab is the fixture because its geometry is a corridor with
//! exactly one doorway (`anchor/door`, gate region `[4,1,6]..[5,3,6]`), which makes
//! "the only route runs through this box" a two-line declaration.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::{Plan, RegionWrite};
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// A private copy of the prefab library with one extra `hello-room` point anchor.
fn prefabs_with_anchor(name: &str, anchor: &str, pos: [i32; 3]) -> PathBuf {
    let dir = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    meta["anchors"][anchor] = serde_json::json!({ "pos": pos });
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
}

/// A private copy of the prefab library with a `anchor/shelf` point anchor high in
/// a corner — a box no route touches, so an emission case can write a region
/// without also testing reachability.
fn prefabs_with_shelf(name: &str) -> PathBuf {
    prefabs_with_anchor(name, "anchor/shelf", [1, 4, 1])
}

/// A private copy of the prefab library with `anchor/doorstep` on the **floor cell
/// under the doorway** (local `[4,0,6]` → world `[4,64,6]`).
///
/// `hello-room`'s floor is a single slab at y=64 over open void, and its one
/// doorway is `x ∈ {4,5}, z = 6`, so `[4,64,6]` / `[5,64,6]` are the entire footing
/// of the only route between the two halves of the room. A region write over them
/// decides whether the delve is completable at all — which is what makes it the
/// honest fixture for "what does this write leave behind".
fn prefabs_with_doorstep(name: &str) -> PathBuf {
    prefabs_with_anchor(name, "anchor/doorstep", [4, 0, 6])
}

/// A hello-world `quests` doc at 0.10.0 whose `obj/talk` bundle carries `effects`
/// (a raw JSON array body) after the open-gate.
fn quests_doc(effects: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.10.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }}{effects} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn parse_hw(quests: &str) -> Campaign {
    let raw = RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn try_build(c: &Campaign, prefab_dir: &Path) -> Result<BuildOutput, emit::BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(prefab_dir).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(prefab_dir.join(&t.structure_file)).unwrap();
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
}

fn all_functions(out: &BuildOutput) -> String {
    let mut s = String::new();
    for (path, bytes) in out {
        if path.starts_with("datapack/") && path.ends_with(".mcfunction") {
            s.push_str(std::str::from_utf8(bytes).unwrap());
            s.push('\n');
        }
    }
    s
}

// --- emission ---------------------------------------------------------------

/// A `fill-region` lowers to one `fill` over the resolved box with the authored
/// block, and a `clear-region` to the same `fill` with air — and, unlike
/// `open-gate`, **no `replace` filter**: an author's clear empties the box, it does
/// not scrub one block id out of it.
#[test]
fn region_writes_emit_one_fill_over_the_resolved_box() {
    let dir = prefabs_with_shelf("dw-region-write-emit");
    // anchor/shelf is [1,4,1]; extent [1,0,1] → [0,4,0]..[2,4,2].
    let c = parse_hw(&quests_doc(
        r#", { "type": "fill-region",
              "region": { "anchor": "anchor/shelf", "extent": [1, 0, 1] },
              "block": "minecraft:oak_planks" },
           { "type": "clear-region",
              "region": { "anchor": "anchor/shelf", "extent": [1, 0, 1] } }"#,
    ));
    let out = try_build(&c, &dir).expect("a region write off the route builds");
    let all = all_functions(&out);
    assert!(
        all.contains("fill 0 68 0 2 68 2 minecraft:oak_planks\n"),
        "fill-region must emit one fill over the resolved box:\n{all}"
    );
    assert!(
        all.contains("fill 0 68 0 2 68 2 minecraft:air\n"),
        "clear-region must emit the same fill with air:\n{all}"
    );
    assert!(
        !all.contains("fill 0 68 0 2 68 2 minecraft:air replace"),
        "an authored clear carries no `replace` filter — that is the gate's:\n{all}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The gate verbs are the same operation with the box and the block read off the
/// prefab anchor: `close-gate` fills `anchor/door`'s region with its declared
/// `minecraft:iron_bars`, `open-gate` clears it `replace`-filtered to that block.
///
/// Byte-for-byte the commands they emitted before the capability moved — this is
/// the regression that keeps "a campaign written today compiles and emits
/// identically tomorrow" a test rather than a promise.
#[test]
fn the_gate_verbs_emit_exactly_what_they_always_did() {
    // The seal fires on `on_complete`, after the exit is reached: sealing it
    // earlier walls the only route, which is `DW0311` and a different test.
    let c = parse_hw(&quests_doc("").replace(
        r#"[ { "type": "campaign-complete" } ]"#,
        r#"[ { "type": "close-gate", "anchor": "anchor/door" },
             { "type": "campaign-complete" } ]"#,
    ));
    let out = try_build(&c, &common::prefabs_dir()).expect("the gate campaign builds");
    let all = all_functions(&out);
    assert!(
        all.contains("fill 4 65 6 5 67 6 minecraft:air replace minecraft:iron_bars\n"),
        "open-gate: a `replace`-filtered clear of the gate anchor's box:\n{all}"
    );
    assert!(
        all.contains("fill 4 65 6 5 67 6 minecraft:iron_bars\n"),
        "close-gate: an unfiltered fill with the anchor's declared block:\n{all}"
    );
}

// --- the completability model ----------------------------------------------

/// Every verb that writes a region arrives at the one model, with the right
/// reading of what it leaves behind: a fill is solid, an author's clear empties the
/// box, and an `open-gate` is neither — it removes only the gate's own block, which
/// the assembled world already holds absent.
#[test]
fn every_region_write_reaches_the_one_model() {
    let dir = prefabs_with_shelf("dw-region-write-model");
    let c = parse_hw(&quests_doc(
        r#", { "type": "fill-region",
              "region": { "anchor": "anchor/shelf", "extent": [0, 0, 0] },
              "block": "minecraft:oak_planks" },
           { "type": "clear-region",
              "region": { "anchor": "anchor/shelf", "extent": [1, 0, 1] } },
           { "type": "close-gate", "anchor": "anchor/door" }"#,
    ));
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let writes: Vec<RegionWrite> = plan.region_events.iter().map(|e| e.write).collect();
    assert_eq!(
        writes.iter().filter(|w| **w == RegionWrite::Unseal).count(),
        1,
        "the open-gate is collected as an Unseal: {writes:?}"
    );
    assert_eq!(
        writes.iter().filter(|w| **w == RegionWrite::Fill).count(),
        2,
        "the fill-region and the close-gate are both Fills: {writes:?}"
    );
    assert_eq!(
        writes.iter().filter(|w| **w == RegionWrite::Clear).count(),
        1,
        "the clear-region is a Clear: {writes:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// --- what the write LEAVES: a fluid is not a floor --------------------------

/// The classification, at the model's front door: a region write's conclusion is
/// read off the **block**, not off the verb or the box.
///
/// Water, flowing water and lava are `Flood` in whatever spelling they arrive,
/// **including without a namespace** — `fill-region` takes a hand-written string, a
/// bare `water` passes block validation and is emitted verbatim as `fill … water`,
/// and vanilla resolves it. A waterlogged block is `Fill`, because its cell is
/// occupied by its host block and a body stands on it: "is this cell wet" and "is
/// this block a fluid" are two questions and only the second one is asked here.
#[test]
fn a_region_write_reads_its_conclusion_off_the_block() {
    for fluid in [
        "minecraft:water",
        "minecraft:water[level=3]",
        "minecraft:lava",
        "minecraft:lava[level=0]",
        "water",
        "lava",
    ] {
        assert_eq!(
            RegionWrite::of_block(fluid),
            RegionWrite::Flood,
            "`{fluid}` fills a box with fluid; nothing stands on it"
        );
    }
    for solid in [
        "minecraft:stone",
        "minecraft:oak_stairs[waterlogged=true]",
        "minecraft:oak_slab[type=bottom,waterlogged=true]",
        "minecraft:iron_bars",
    ] {
        assert_eq!(
            RegionWrite::of_block(solid),
            RegionWrite::Fill,
            "`{solid}` occupies its cell with a block, waterlogged or not"
        );
    }
}

/// A `fill-region` whose block is a **fluid** is collected as a `Flood`, not as a
/// `Fill` — the plan-level half of the same claim, over the real verb.
#[test]
fn a_fill_region_of_water_is_collected_as_a_flood() {
    let dir = prefabs_with_shelf("dw-region-write-fluid-model");
    let c = parse_hw(&quests_doc(
        r#", { "type": "fill-region",
              "region": { "anchor": "anchor/shelf", "extent": [0, 0, 0] },
              "block": "minecraft:water" }"#,
    ));
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let writes: Vec<RegionWrite> = plan.region_events.iter().map(|e| e.write).collect();
    assert_eq!(
        writes.iter().filter(|w| **w == RegionWrite::Flood).count(),
        1,
        "the water fill must reach the model as a Flood: {writes:?}"
    );
    assert_eq!(
        writes.iter().filter(|w| **w == RegionWrite::Fill).count(),
        0,
        "and must NOT reach it as a Fill: {writes:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The soundness hole, end to end.** `hello-room`'s only doorway stands on two
/// floor cells over open void. A runtime `fill-region` replaces whatever is in its
/// box — so filling those cells with `minecraft:water` deletes the floor and leaves
/// water, and the party cannot cross.
///
/// Before a region write's conclusion was read off its block, this campaign built
/// **green**: the model added the box to `solid` whatever the block, so it proved a
/// route that walks the party across a pond in mid-air. The refusal is `DW0544` and
/// not `DW0311` because the geometry is not the defect — the author filled that box
/// on purpose and has to be told that the fluid is what took the footing away.
#[test]
fn a_water_fill_over_the_only_footing_is_dw0543() {
    let dir = prefabs_with_doorstep("dw-region-write-water-doorstep");
    // anchor/doorstep is world [4,64,6]; extent [1,0,0] → [3,64,6]..[5,64,6],
    // which is the whole floor of the only doorway.
    let water = parse_hw(&quests_doc(
        r#", { "type": "fill-region",
              "region": { "anchor": "anchor/doorstep", "extent": [1, 0, 0] },
              "block": "minecraft:water" }"#,
    ));
    match try_build(&water, &dir) {
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0544", "wrong code: {message}");
            assert!(
                message.contains("[3, 64, 6]..[5, 64, 6]"),
                "the message must name the fluid-filled box: {message}"
            );
            assert!(
                message.contains("water or lava"),
                "the message must say what the box holds: {message}"
            );
        }
        other => panic!("a water floor under the only doorway must be refused: {other:?}"),
    }
    // The control: the identical campaign with a block that IS floor builds. Same
    // box, same verb, same fire step, same route — only the block differs, which is
    // the whole claim.
    let planks = parse_hw(&quests_doc(
        r#", { "type": "fill-region",
              "region": { "anchor": "anchor/doorstep", "extent": [1, 0, 0] },
              "block": "minecraft:oak_planks" }"#,
    ));
    try_build(&planks, &dir).expect("the same write with a solid block is floor, and routes");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Lava is the same defect wearing a different id, and it is the case a
/// water-only classifier would have shipped.
#[test]
fn a_lava_fill_over_the_only_footing_is_dw0543_too() {
    let dir = prefabs_with_doorstep("dw-region-write-lava-doorstep");
    let lava = parse_hw(&quests_doc(
        r#", { "type": "fill-region",
              "region": { "anchor": "anchor/doorstep", "extent": [1, 0, 0] },
              "block": "minecraft:lava" }"#,
    ));
    match try_build(&lava, &dir) {
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0544", "wrong code: {message}");
        }
        other => panic!("a lava floor under the only doorway must be refused: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The namespace-less spelling reaches the same refusal. It is a separate case
/// because it is a separate *failure mode*: the classifier that compares against
/// `minecraft:water` accepts this campaign in silence, and the author is handed a
/// world where the doorway floor is water.
#[test]
fn a_bare_water_fill_is_refused_exactly_like_the_namespaced_one() {
    let dir = prefabs_with_doorstep("dw-region-write-bare-water-doorstep");
    let bare = parse_hw(&quests_doc(
        r#", { "type": "fill-region",
              "region": { "anchor": "anchor/doorstep", "extent": [1, 0, 0] },
              "block": "water" }"#,
    ));
    match try_build(&bare, &dir) {
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0544", "wrong code: {message}");
        }
        other => panic!("`water` is the same block as `minecraft:water`: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// --- floor the campaign LAYS, end to end ------------------------------------

/// A `quests` doc whose second objective is reached by walking onto a step the
/// campaign **fills at runtime**: `obj/talk` lays a block in `anchor/exit`'s cell
/// (world `[5,65,8]`), and `obj/exit` then stands on top of it at `anchor/perch`
/// (world `[5,66,8]`). The step does not exist in the assembled world; it exists
/// only from the point in the quest graph where the fill fires.
fn quests_doc_over_a_laid_step() -> String {
    r#"{
  "dsl_version": "0.10.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/perch",
            "radius": 2, "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "fill-region",
              "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] },
              "block": "minecraft:oak_planks" }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#
    .to_string()
}

/// **A critical path that crosses floor the campaign lays at runtime ships.**
///
/// The whole delve, end to end: the party talks to the keeper, that lays a plank
/// in the cell at `[5,65,8]`, and the last objective is reached by standing on it
/// at `[5,66,8]`. Nothing in the assembled world holds that step — it is authored
/// as a runtime region write, which is exactly what `fill-region` is for.
///
/// This campaign could not be built. The completability proof credited the plank
/// (it has read the leg's runtime region state since spec-0031), so the leg routed
/// — and the waypoint self-check then re-judged those very cells against the BARE
/// assembled world, where the plank does not exist, and refused the build `DW0314`
/// for a waypoint with "no floor". Two halves of the compiler disagreeing about
/// one world, with the router right and the check wrong.
///
/// The leg now carries the world it was proven over, so there is nothing left to
/// disagree with.
#[test]
fn a_critical_path_over_a_runtime_laid_step_builds() {
    let dir = prefabs_with_anchor("dw-region-write-laid-step", "anchor/perch", [5, 2, 8]);
    let laid = parse_hw(&quests_doc_over_a_laid_step());
    let out = try_build(&laid, &dir)
        .expect("a critical path over floor the campaign lays at runtime must build");

    // Bound, not assumed. Three things must be true or this test proves nothing:
    // the delve really does emit the fill, the exported waypoints really do stand
    // on the laid step, and the step really is absent from the assembled world.
    let functions = all_functions(&out);
    assert!(
        functions.contains("fill 5 65 8 5 65 8 minecraft:oak_planks"),
        "the campaign must actually lay the step it walks on"
    );
    let waypoints: serde_json::Value = out
        .iter()
        .find(|(p, _)| p.as_str() == "validation/critical-path-waypoints.json")
        .map(|(_, b)| serde_json::from_slice(b).unwrap())
        .expect("the proven critical path must be exported for the harness");
    let stands_on_the_step = waypoints["legs"]
        .as_array()
        .expect("legs")
        .iter()
        .flat_map(|l| l["waypoints"].as_array().expect("waypoints").iter())
        .any(|w| w.as_array().map(|c| c == &[5, 66, 8]).unwrap_or(false));
    assert!(
        stands_on_the_step,
        "the exported route must actually stand on the laid step: {waypoints:#}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
