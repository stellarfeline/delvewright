//! DSL v0.6 `close-gate` — the physical dual of `open-gate` (island boulder seal).
//! Covers the gate-block validation (`DW0343`), the fill-with-declared-block
//! emission, and that a `close-gate` on a block-bearing gate validates + builds.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::gates;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign, validate_campaign_with};

/// A hello-world `quests` doc that opens `anchor/door` on the talk objective and
/// runs `on_complete` (a raw JSON array body, no surrounding brackets) after the
/// exit is reached — where a `close-gate` seals nothing still to be walked.
fn quests_doc(on_complete: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.6.0",
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
        "on_complete": [ {on_complete} ]
      }}
    ]
  }}
}}"#
    )
}

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn parse_hw(quests: &str) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read_hw("dialogue.json"),
    };
    parse_campaign(&raw).expect("campaign parses")
}

/// Emit a hello-world campaign (with the given `quests` doc) and return the build
/// output. A clean build proves the whole pipeline — including the close-gate nav
/// model — passes.
fn build(campaign: &Campaign, prefabs: &PrefabRegistry) -> BuildOutput {
    let plan = Plan::build(campaign, prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &tree,
        prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
        delvewright_compiler::light::has_night_vision(campaign),
    )
    .expect("every emitted command validates; the close-gate nav proof passes")
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

// --- DW0343: close-gate on a gate anchor that declares no block ------------

/// A `close-gate` on a block-declaring gate anchor (`anchor/door` → `iron_bars`)
/// passes the gate-block check.
#[test]
fn close_gate_with_declared_block_validates_clean() {
    let c = parse_hw(&quests_doc(
        r#"{ "type": "close-gate", "anchor": "anchor/door" },
           { "type": "campaign-complete" }"#,
    ));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    assert!(
        gates::check_close_gates(&c, &prefabs).is_empty(),
        "close-gate on a block-bearing gate anchor must validate clean"
    );
}

/// A `close-gate` on a gate anchor whose prefab metadata declares no fill `block`
/// is `DW0343`. The prefab library is copied to a private temp dir and the
/// `anchor/door` block stripped (the real content repo is read-only).
#[test]
fn close_gate_on_blockless_gate_is_dw0343() {
    let tmp = std::env::temp_dir().join("dw-close-gate-noblock-prefabs");
    let _ = std::fs::remove_dir_all(&tmp);
    common::copy_dir_all(&common::prefabs_dir(), &tmp);
    // Strip the `block` from hello-room's `anchor/door` gate.
    let hello = tmp.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&hello).unwrap()).unwrap();
    let removed = meta["anchors"]["anchor/door"]
        .as_object_mut()
        .unwrap()
        .remove("block");
    assert!(removed.is_some(), "fixture must have had a block to strip");
    std::fs::write(&hello, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

    let c = parse_hw(&quests_doc(
        r#"{ "type": "close-gate", "anchor": "anchor/door" },
           { "type": "campaign-complete" }"#,
    ));
    let prefabs = PrefabRegistry::load_dir(&tmp).unwrap();
    let d = gates::check_close_gates(&c, &prefabs);
    assert!(
        d.iter().any(|x| x.code == gates::DW_GATE_NO_BLOCK),
        "a close-gate on a blockless gate anchor must be DW0343: {d:#?}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

// --- emission: fill the region with the anchor's declared block ------------

/// `close-gate` fills the gate region with the block the anchor declares (the dual
/// of `open-gate`'s `fill … air`): `anchor/door` → `fill … minecraft:iron_bars`.
#[test]
fn close_gate_emits_fill_with_declared_block() {
    let c = parse_hw(&quests_doc(
        r#"{ "type": "close-gate", "anchor": "anchor/door" },
           { "type": "campaign-complete" }"#,
    ));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    // Validation-tier gate-block check passes (anchor/door declares iron_bars).
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let mut diags = validate_campaign_with(&c, &items, &prefabs, &entities);
    diags.extend(gates::check_close_gates(&c, &prefabs));
    assert!(
        diags.is_empty(),
        "close-gate campaign validates: {diags:#?}"
    );

    let out = build(&c, &prefabs);
    let all = all_functions(&out);
    // The sealing fill uses the declared block and, unlike open-gate, no `air
    // replace` clause (it lays the block over the whole region).
    assert!(
        all.contains("minecraft:iron_bars"),
        "close-gate must fill with the anchor's declared block; functions:\n{all}"
    );
    let sealed = all.lines().any(|l| {
        l.starts_with("fill ") && l.ends_with(" minecraft:iron_bars") && !l.contains("replace")
    });
    assert!(
        sealed,
        "close-gate must emit a plain `fill … iron_bars`:\n{all}"
    );
}

/// A `close-gate` and `open-gate` nested inside a `sequence` step are collected as
/// gate events (via the shared `visit_deep`/`nested_effect_lists` authority), so the
/// close-gate completability model sees a sequence-nested seal AND its reopen —
/// the fix for the island's nested boulder seal/reopen. Regression for the DW0311
/// gate-ordering scan being top-level-only.
#[test]
fn nested_sequence_gate_effects_are_collected() {
    let c = parse_hw(&quests_doc(
        r#"{ "type": "sequence", "steps": [
              { "at_ticks": 0, "effects": [ { "type": "close-gate", "anchor": "anchor/door" } ] },
              { "at_ticks": 40, "effects": [ { "type": "open-gate", "anchor": "anchor/door" } ] }
           ] },
           { "type": "campaign-complete" }"#,
    ));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds with nested gate effects");
    let closes: Vec<bool> = plan.gate_events.iter().map(|e| e.closes).collect();
    assert!(
        closes.contains(&true),
        "the sequence-nested close-gate must be collected: {closes:?}"
    );
    assert!(
        closes.contains(&false),
        "the sequence-nested open-gate must be collected: {closes:?}"
    );
}
