//! **The root of runtime-laid footing decides both the completability proof and
//! the exported route — and the two answers are one answer.**
//!
//! Two independent changes meet here, and neither could be tested on its own
//! branch because on each branch only half of this existed.
//!
//! * A route leg carries the world it was **proven over**, so the exported-waypoint
//!   self-check (`DW0314`) judges each leg in that world rather than in the bare
//!   assembled one. That is what lets a critical path cross floor the campaign lays
//!   at runtime: the plank is not in the assembled model, so judging the bare world
//!   called its cells "no floor" and refused a correct campaign.
//! * The completability proof credits footing **only** from a write the party
//!   cannot skip, decided at the effect's root. An unforced fill reads as impassable
//!   and not floor, and a forced leg that leans on one is `DW0546`.
//!
//! They meet mechanically: the per-leg proven world is built by the same
//! region-state construction (`World::with_region_state`) that now also applies the
//! unforced reading. So the export self-check inherits forcedness without anybody
//! writing that down, and the campaign-scale statement of it is this file.
//!
//! **The fixture.** `hello-room`'s one doorway (`x ∈ {4,5}, z = 6`) stands on the
//! floor cells `[4,64,6]..[5,64,6]`, and they are the entire footing of the only
//! route between the two halves of the room. A stage-7 world edit replaces them
//! with an oak fence — something the built world genuinely holds, that a body can
//! neither pass through nor stand on top of — so the doorway has no floor **in the
//! assembled world**, which is the state the export self-check used to judge
//! against. The campaign then lays planks over the same box at runtime.
//!
//! Three builds of one campaign, differing only in where that fill hangs.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// A private copy of the prefab library with one extra `hello-room` point anchor
/// on the floor of the only doorway: `anchor/doorstep` (local `[4,0,6]` → world
/// `[4,64,6]`). One anchor is enough — the world edit and the runtime fill are
/// deliberately the **same box**, so that the only thing under test is the root.
fn prefabs_with_doorstep(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    meta["anchors"]["anchor/doorstep"] = serde_json::json!({ "pos": [4, 0, 6] });
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
}

/// The stage-7 edit that takes the doorway floor out of the **built** world:
/// `[4,64,6]..[5,64,6]` becomes oak fence. A fence blocks passage and is never
/// floor, so the doorway is impassable — and, unlike air, it leaves no void column
/// for the boundary-safety proof (`DW0322`) to find, which keeps this fixture about
/// footing and nothing else.
fn fence_the_doorway() -> String {
    serde_json::json!({
        "dsl_version": "0.19.0",
        "campaign_id": "hello-world",
        "stage": "world-edits",
        "content": { "batches": [ {
            "id": "batch/fence-the-doorway",
            "area": "area/keep",
            "note": "the built world has no floor in the doorway, only a rail",
            "edits": [
                { "verb": "select", "name": "region/doorway-floor", "shape": {
                    "kind": "box",
                    "frame": { "kind": "anchor-relative", "anchor": "anchor/doorstep" },
                    "min": [0, 0, 0], "max": [1, 0, 0] } },
                { "verb": "fill", "region": "region/doorway-floor", "recipe": {
                    "blocks": [ { "block": "minecraft:oak_fence", "weight": 1.0 } ] } }
            ]
        } ] }
    })
    .to_string()
}

/// The runtime write that lays the doorway floor: one `fill-region` of planks over
/// `anchor/doorstep ± [1,0,0]` → `[3,64,6]..[5,64,6]`, which is the box the edit
/// fenced plus the one cell beside it the room already floors.
const LAY_THE_FLOOR: &str = r#"{ "type": "fill-region",
        "region": { "anchor": "anchor/doorstep", "extent": [1, 0, 0] },
        "block": "minecraft:oak_planks" }"#;

/// A hello-world `quests` doc with the floor-laying fill spliced into exactly one
/// root: the keeper's `on_objective_complete` bundle, which the party is forced
/// through, or the campaign's `on_death` bundle, which runs only if somebody dies.
fn quests_doc(forced_extra: &str, on_death: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.19.0",
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }}{forced_extra} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "on_death": [ {on_death} ]
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
        world_edits: Some(fence_the_doorway()),
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
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

/// A two-quest stage-4 plan: the hello-world finale,
/// plus a second quest whose `mandatory` this arm varies. The second quest
/// depends on nothing and nothing depends on it, so when it is declared
/// optional it is legally off the finale's closure (spec-0051 §4).
fn plan_with_second_quest(mandatory: bool) -> String {
    format!(
        r#"{{
  "dsl_version": "0.19.0",
  "campaign_id": "hello-world",
  "stage": "quest-plan",
  "content": {{
    "finale": "quest/open-the-door",
    "quests": [
      {{ "id": "quest/open-the-door", "goal": "Get the Keeper to open the door and leave the keep.",
         "area": "area/keep", "npcs": ["npc/keeper"], "depends_on": [], "mandatory": true, "act": 1 }},
      {{ "id": "quest/side", "goal": "Look at the doorstep.",
         "area": "area/keep", "npcs": [], "depends_on": [], "mandatory": {mandatory}, "act": 1 }}
    ]
  }}
}}"#
    )
}

/// The same two-quest campaign, with `LAY_THE_FLOOR` hung on the SECOND quest's
/// `on_complete`. Only the stage-4 `mandatory` differs between the two calls.
fn parse_two_quest(mandatory: bool) -> Campaign {
    let quests = format!(
        r#"{{
  "dsl_version": "0.19.0",
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }},
      {{
        "id": "quest/side",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "reach-anchor", "id": "obj/doorstep", "anchor": "anchor/doorstep", "radius": 2 }}
        ],
        "on_complete": [ {LAY_THE_FLOOR} ]
      }}
    ],
    "on_death": []
  }}
}}"#
    );
    let raw = RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: plan_with_second_quest(mandatory),
        quests,
        dialogue: hw("dialogue.json"),
        world_edits: Some(fence_the_doorway()),
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

/// spec-0051 §8.6 — **the skippable-root class reaches optional quests.**
///
/// One declaration varied, nothing else. A fill hung on an optional quest's
/// `on_complete` is a fill the party may never cause, so it seals and lays no
/// footing; the same fill on the same box hung on a MANDATORY quest is forced
/// and carries the route.
///
/// The red arm is the one only this rule could produce. Before spec-0051
/// `EffectRoot::QuestComplete` was hard-coded forced, so this campaign BUILT —
/// and shipped a delve whose only route stands on planks laid by content
/// nobody has to play.
#[test]
fn an_optional_quests_fill_lays_no_footing_the_forced_path_may_stand_on() {
    let dir = prefabs_with_doorstep("dw-optional-footing-root");

    match try_build(&parse_two_quest(false), &dir) {
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0546", "wrong code: {message}");
            assert!(
                message.contains("quest/side"),
                "the refusal must name the OPTIONAL quest that lays the footing: {message}"
            );
            assert!(
                message.contains("[3, 64, 6]..[5, 64, 6]"),
                "the refusal must name the box it lays: {message}"
            );
        }
        other => panic!(
            "the only route may not stand on floor laid by an OPTIONAL quest — this is \
             the spec-0051 §8.6 widening, and a pass here means the skippable class \
             never reached the new root: {other:?}"
        ),
    }

    // The identical fill, the identical box, one word changed in stage 4.
    try_build(&parse_two_quest(true), &dir).expect(
        "the same fill hung on a MANDATORY quest is forced and must carry the route — \
         if this fails, the red arm above proved nothing about optionality",
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// **The junction, at campaign scale: one campaign, three roots, three verdicts.**
///
/// Same box, same block, same verb, same anchors, same route. The only thing that
/// differs between the three builds below is where the fill hangs — and that alone
/// decides whether the delve is refused as unroutable, refused as leaning on a beat
/// nobody has to play, or shipped with a proven route across it.
///
/// Each verdict pins one half of the merge, and the two halves fail in opposite
/// directions, which is why they are one test:
///
/// * **The forced build reds if a leg stops carrying the world it was proven over.**
///   The doorway cells have no floor in the assembled world — the edit fenced them —
///   so the exported route across them is refused `DW0314` the moment the self-check
///   goes back to judging the bare world.
/// * **The `on_death` build reds if an unforced fill goes back to being ordinary
///   floor.** It builds green instead, and ships a delve whose only route stands on
///   planks that a party which never died would find missing.
///
/// **What would make this test vacuous**, stated so a later reader can check it
/// rather than take it on trust:
///
/// * If the doorway were passable without the fill, all three builds would be about
///   a plank nobody needs. The first build is that control, and it is a refusal:
///   with no fill anywhere the delve is `DW0311`, unroutable.
/// * If a refusal came from some unrelated defect it would look right and mean
///   nothing, so each is pinned to its code — never merely to "an error" — and the
///   `DW0546` message is required to name both the beat that lays the footing and
///   the box it lays.
/// * If the forced build succeeded without its route ever crossing the doorway, the
///   accept would be about a leg that never touches the laid floor. So the emitted
///   `fill` and the exported critical path are both read off the built artifact, and
///   the exported leg is required to span the doorway.
#[test]
fn the_root_of_runtime_laid_footing_decides_the_proof_and_the_export() {
    let dir = prefabs_with_doorstep("dw-laid-footing-root");

    // --- the control: nothing lays the floor, and the delve is unroutable -----
    match try_build(&parse_hw(&quests_doc("", "")), &dir) {
        Err(emit::BuildFailure::Diagnostic { code, message }) => assert_eq!(
            code, "DW0311",
            "a doorway floored only by a fence must be an ordinary unroutable leg: {message}"
        ),
        other => panic!(
            "the fixture must be BINDING — without the runtime fill there is no way \
             through the doorway: {other:?}"
        ),
    }

    // --- the optional root: refused, and the beat is named --------------------
    match try_build(&parse_hw(&quests_doc("", LAY_THE_FLOOR)), &dir) {
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0546", "wrong code: {message}");
            assert!(
                message.contains("on_death"),
                "the refusal must name the beat that lays the footing: {message}"
            );
            assert!(
                message.contains("[3, 64, 6]..[5, 64, 6]"),
                "the refusal must name the box that lays the footing: {message}"
            );
        }
        other => panic!(
            "the only route may not stand on floor laid by a beat nobody has to \
             play: {other:?}"
        ),
    }

    // --- the forced root: the identical fill, and the delve ships --------------
    let out = try_build(
        &parse_hw(&quests_doc(&format!(", {LAY_THE_FLOOR}"), "")),
        &dir,
    )
    .expect(
        "floor laid by a beat the party cannot skip must carry the path AND pass the \
                 exported-waypoint self-check",
    );
    assert!(
        all_functions(&out).contains("fill 3 64 6 5 64 6 minecraft:oak_planks"),
        "the campaign must actually lay the floor it walks on"
    );
    let waypoints: serde_json::Value = out
        .iter()
        .find(|(p, _)| p.as_str() == "validation/critical-path-waypoints.json")
        .map(|(_, b)| serde_json::from_slice(b).unwrap())
        .expect("the proven critical path must be exported for the harness");
    // The doorway is at z = 6. A leg with waypoints on both sides of it is a leg
    // whose proven polyline crosses the cells whose floor is laid at runtime —
    // which is what the export self-check accepted.
    assert!(
        waypoints["legs"].as_array().expect("legs").iter().any(|l| {
            let z: Vec<i64> = l["waypoints"]
                .as_array()
                .expect("waypoints")
                .iter()
                .filter_map(|w| w.as_array().and_then(|c| c[2].as_i64()))
                .collect();
            z.iter().any(|&v| v < 6) && z.iter().any(|&v| v > 6)
        }),
        "the exported route must cross the doorway whose floor is laid at runtime: {waypoints:#}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
