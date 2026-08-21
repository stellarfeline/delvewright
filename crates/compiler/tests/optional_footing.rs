//! **Footing the critical path stands on must come from a firing the party cannot
//! skip.**
//!
//! `hello-room`'s only doorway stands on the floor cells `[3,64,6]..[5,64,6]`; take
//! them away and the two halves of the room are separated by open void. The campaign
//! below takes them away on a beat the party is *forced* through (the keeper's
//! `on_objective_complete` bundle clears the box) and lays one cell of them back from
//! a **trap payload** — a bundle nobody is forced to spring.
//!
//! The completability model registers a fill from an optional root because a fill is
//! conservative *for passage*: assuming a wall is there can only make the proof
//! harder. It is not conservative for **footing**, and that is the asymmetry these
//! tests pin: the same solid block that walls a doorway also floors the cell above
//! it, so an unforced fill was being leaned on to carry the forced path.
//!
//! The party that never opens the trapped chest walks to the doorway and finds void.

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

/// A private copy of the prefab library with two extra `hello-room` point anchors on
/// the floor of the only doorway: `anchor/doorstep` (local `[4,0,6]` → world
/// `[4,64,6]`) and `anchor/plank` (local `[5,0,6]` → world `[5,64,6]`).
///
/// Two anchors because the model keys a runtime write by its **box**, so a clear over
/// `[3,64,6]..[5,64,6]` and a fill over `[5,64,6]` are two independent writes rather
/// than one overwriting the other — which is what lets the fixture state "the forced
/// beat takes the floor, the optional beat puts one cell of it back".
fn prefabs_with_doorway_anchors(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    meta["anchors"]["anchor/doorstep"] = serde_json::json!({ "pos": [4, 0, 6] });
    meta["anchors"]["anchor/plank"] = serde_json::json!({ "pos": [5, 0, 6] });
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
}

/// The forced beat: completing `obj/talk` opens the door **and** clears the whole
/// doorway floor out from under it.
const FORCED_CLEAR: &str = r#", { "type": "clear-region",
        "region": { "anchor": "anchor/doorstep", "extent": [1, 0, 0] } }"#;

/// A hello-world `quests` doc with an optional `traps` body and an extra forced
/// effect bundle spliced in.
fn quests_doc(forced_extra: &str, traps: &str) -> String {
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }}{forced_extra} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "traps": [ {traps} ]
  }}
}}"#
    )
}

/// A `fill-region` of a solid block over `[5,64,6]`, spelled as the body of whichever
/// root the caller is testing.
const PLANK_FILL: &str = r#"{ "type": "fill-region",
        "region": { "anchor": "anchor/plank", "extent": [0, 0, 0] },
        "block": "minecraft:oak_planks" }"#;

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
        site_plan: None,
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

/// The control that gives the fixture its meaning: with the forced clear and **no
/// plank at all**, the doorway has no floor and the delve is refused. Whatever the
/// two tests below conclude, they conclude it about a campaign that is broken without
/// the fill.
#[test]
fn the_forced_clear_alone_strands_the_party() {
    let dir = prefabs_with_doorway_anchors("dw-optional-footing-control");
    let c = parse_hw(&quests_doc(FORCED_CLEAR, ""));
    match try_build(&c, &dir) {
        Err(emit::BuildFailure::Diagnostic { code, .. }) => {
            assert_eq!(
                code, "DW0311",
                "clearing the only doorway floor must be an ordinary unroutable leg"
            );
        }
        other => panic!("a doorway over void must be refused: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The reproduction.** The plank is laid from a `traps[].payload` — a bundle the
/// party may never spring — and it is the only footing the forced leg has.
#[test]
fn optional_footing_on_the_forced_path_is_refused() {
    let dir = prefabs_with_doorway_anchors("dw-optional-footing-trap");
    let traps = format!(
        r#"{{
      "id": "trap/plank-drop",
      "at": "anchor/exit",
      "trigger": "trapped-chest",
      "lethality": "nonlethal",
      "payload": [ {PLANK_FILL} ]
    }}"#
    );
    let c = parse_hw(&quests_doc(FORCED_CLEAR, &traps));
    match try_build(&c, &dir) {
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0546", "wrong code: {message}");
            assert!(
                message.contains("trap/plank-drop"),
                "the message must name the beat that lays the footing: {message}"
            );
            assert!(
                message.contains("[5, 64, 6]"),
                "the message must name the box that lays the footing: {message}"
            );
        }
        other => panic!("footing laid from a trap payload may not carry a forced leg: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The other direction, and the one that makes the fix a fix rather than a second
/// blanket refusal: the **identical** campaign with the plank moved onto the keeper's
/// own forced bundle builds. Same box, same block, same verb, same route — only the
/// root differs, which is the whole claim.
#[test]
fn the_same_plank_from_a_forced_beat_builds() {
    let dir = prefabs_with_doorway_anchors("dw-optional-footing-forced");
    let forced = format!("{FORCED_CLEAR}, {PLANK_FILL}");
    let c = parse_hw(&quests_doc(&forced, ""));
    try_build(&c, &dir).expect("footing laid by a beat the party cannot skip carries the path");
    let _ = std::fs::remove_dir_all(&dir);
}
