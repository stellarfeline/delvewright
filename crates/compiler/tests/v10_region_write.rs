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

/// A private copy of the prefab library with a `anchor/shelf` point anchor high in
/// a corner — a box no route touches, so an emission case can write a region
/// without also testing reachability.
fn prefabs_with_shelf(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    meta["anchors"]["anchor/shelf"] = serde_json::json!({ "pos": [1, 4, 1] });
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
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
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn try_build(c: &Campaign, prefab_dir: &Path) -> Result<BuildOutput, emit::BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(prefab_dir).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(prefab_dir.join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
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
