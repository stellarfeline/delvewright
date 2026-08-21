//! Emission-layer guards from the audit batch: the generated-name collision check
//! (`DW0361`), the dialogue variant cap (`DW0362`), the campaign-derived
//! completion outro, and the mannequin `pose` NBT.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// The hello-world campaign with the named stage documents replaced.
fn parse_hw_with(overrides: &[(&str, String)]) -> Campaign {
    let get = |name: &str| {
        overrides
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| read_hw(name))
    };
    parse_campaign(&RawCampaign {
        world: get("world.json"),
        npcs: get("npcs.json"),
        classes: get("classes.json"),
        quest_plan: get("quest-plan.json"),
        quests: get("quests.json"),
        dialogue: get("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    })
    .expect("campaign parses")
}

fn try_build(campaign: &Campaign) -> Result<BuildOutput, BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
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

fn build_ok(campaign: &Campaign) -> BuildOutput {
    try_build(campaign).expect("build succeeds")
}

fn fn_body(out: &BuildOutput, needle: &str) -> String {
    let (_, bytes) = out
        .iter()
        .find(|(p, _)| p.starts_with("datapack/") && p.contains(needle))
        .unwrap_or_else(|| panic!("no emitted artifact matching `{needle}`"));
    String::from_utf8(bytes.clone()).unwrap()
}

// --- DW0361: generated-name collision ---------------------------------------

/// A `quests` doc carrying a wave whose id sanitizes into the same generated
/// function name as a deferred NPC's summon: wave `wave/npc-x` → `spawn_npc_x`,
/// npc `npc/x` → `spawn_npc_x`. `safe_local` drops the `<kind>/` prefix and folds
/// `-` to `_`, so the two ids collide even though nothing about them looks alike.
const COLLIDING_QUESTS: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "waves": [
      { "id": "wave/npc-x", "anchor": "anchor/exit",
        "mobs": [ { "entity": "minecraft:zombie", "count": 1 } ] }
    ],
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "kill", "id": "obj/fight", "wave": "wave/npc-x", "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "spawn-npc", "npc": "npc/x" },
            { "type": "spawn-wave", "wave": "wave/npc-x" }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

/// `dialogue` with a (minimal) tree for the second NPC — every stage-2 NPC needs
/// one (cross-stage 1:1), and only NPCs with a tree reach the emitter's npc plan.
fn dialogue_with_x() -> String {
    let mut v: serde_json::Value = serde_json::from_str(&read_hw("dialogue.json")).unwrap();
    let trees = v["content"]["dialogues"].as_array_mut().unwrap();
    trees.push(serde_json::json!({
        "npc": "npc/x",
        "root": "dlg/x-greeting",
        "nodes": [ { "id": "dlg/x-greeting", "text": "Nothing to say.", "options": [] } ]
    }));
    serde_json::to_string(&v).unwrap()
}

/// `npcs` with a second, `deferred` NPC whose id sanitizes to `x`.
fn npcs_with_deferred_x() -> String {
    let mut v: serde_json::Value = serde_json::from_str(&read_hw("npcs.json")).unwrap();
    let npcs = v["content"]["npcs"].as_array_mut().unwrap();
    let mut extra = npcs[0].clone();
    extra["id"] = "npc/x".into();
    extra["name"] = "X".into();
    extra["anchor"] = "anchor/exit".into();
    extra["deferred"] = true.into();
    npcs.push(extra);
    v["dsl_version"] = "0.6.0".into();
    serde_json::to_string(&v).unwrap()
}

/// Two generated functions that sanitize to the same name is `DW0361`, not a
/// silent `BTreeMap` overwrite in which the wave simply never spawns.
#[test]
fn colliding_generated_names_are_dw0361() {
    let c = parse_hw_with(&[
        ("quests.json", COLLIDING_QUESTS.to_string()),
        ("npcs.json", npcs_with_deferred_x()),
        ("dialogue.json", dialogue_with_x()),
    ]);
    match try_build(&c) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0361", "wrong code; message was: {message}");
            assert!(
                message.contains("spawn_npc_x"),
                "the diagnostic must name the colliding artifact: {message}"
            );
        }
        Err(other) => panic!("expected DW0361, got {other:?}"),
        Ok(_) => panic!("expected DW0361, but the build succeeded"),
    }
}

/// The control: rename the wave so nothing collides and the same campaign builds,
/// emitting both functions. Proves `DW0361` fires on the collision, not on the
/// shape of the campaign.
#[test]
fn non_colliding_names_build_and_emit_both() {
    let quests = COLLIDING_QUESTS.replace("wave/npc-x", "wave/ambush");
    let c = parse_hw_with(&[
        ("quests.json", quests),
        ("npcs.json", npcs_with_deferred_x()),
        ("dialogue.json", dialogue_with_x()),
    ]);
    let out = build_ok(&c);
    for name in ["spawn_npc_x.mcfunction", "spawn_ambush.mcfunction"] {
        assert!(
            out.keys().any(|p| p.ends_with(name)),
            "expected `{name}` in the emitted pack"
        );
    }
}

// --- DW0362: dialogue variant cap -------------------------------------------

/// The hello-world `quests` doc at `dsl_version 0.4.0` — the variant-dialog
/// encoding (and so the cap) only exists for v0.4+ campaigns.
fn quests_v04() -> String {
    let mut v: serde_json::Value = serde_json::from_str(&read_hw("quests.json")).unwrap();
    v["dsl_version"] = "0.4.0".into();
    serde_json::to_string(&v).unwrap()
}

/// `dialogue` whose greeting node carries `n` flag-gated options on top of the
/// two it ships with. Vanilla cannot hide a dialog option, so the compiler
/// precomputes `2^n` variants — and used to compute the bitmask with `1u32 << i`,
/// which panics outright at 32 options.
fn dialogue_with_gated_options(n: usize) -> String {
    let mut v: serde_json::Value = serde_json::from_str(&read_hw("dialogue.json")).unwrap();
    v["dsl_version"] = "0.4.0".into();
    let opts = v["content"]["dialogues"][0]["nodes"][0]["options"]
        .as_array_mut()
        .unwrap();
    for i in 0..n {
        opts.push(serde_json::json!({
            "label": format!("Option {i}"),
            "next": "dlg/lore",
            "requires_flags": [format!("flag/g-{i}")],
        }));
    }
    serde_json::to_string(&v).unwrap()
}

/// A node past the cap is `DW0362` — a coded content diagnostic naming the node,
/// where 32 gated options used to be an outright compiler panic and everything
/// below that a silent `2^n` pack explosion.
#[test]
fn too_many_gated_dialogue_options_is_dw0362() {
    let c = parse_hw_with(&[
        ("dialogue.json", dialogue_with_gated_options(11)),
        ("quests.json", quests_v04()),
    ]);
    match try_build(&c) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0362", "wrong code; message was: {message}");
            assert!(
                message.contains("dlg/greeting"),
                "the diagnostic must name the offending node: {message}"
            );
        }
        Err(other) => panic!("expected DW0362, got {other:?}"),
        Ok(_) => panic!("expected DW0362, but the build succeeded"),
    }
}

/// A node **at** the cap still builds — the limit is exactly where it is
/// documented, and no shipped campaign comes near it (the largest gates four).
#[test]
fn gated_dialogue_options_at_the_cap_build() {
    // The greeting node already carries one gated option (a `complete-objective`),
    // so nine more lands exactly on the cap of ten.
    let c = parse_hw_with(&[
        ("dialogue.json", dialogue_with_gated_options(9)),
        ("quests.json", quests_v04()),
    ]);
    build_ok(&c);
}

// --- campaign-derived completion advancement --------------------------------

/// The completion advancement's description used to be the literal string
/// `"You left the keep."` in **every** delve ever built — the reference
/// keep-crawl's line, shipped to campaigns that have no keep and untranslatable
/// in any sidecar. It is now the finale quest's goal by default.
#[test]
fn completion_advancement_description_is_campaign_derived() {
    let out = build_ok(&parse_hw_with(&[]));
    let adv = fn_body(&out, "advancement/campaign_complete.json");
    assert!(
        !adv.contains("You left the keep."),
        "the hardcoded keep line must be gone:\n{adv}"
    );
    let finale_goal = {
        let plan: serde_json::Value = serde_json::from_str(&read_hw("quest-plan.json")).unwrap();
        let finale = plan["content"]["finale"].as_str().unwrap().to_string();
        plan["content"]["quests"]
            .as_array()
            .unwrap()
            .iter()
            .find(|q| q["id"] == finale.as_str())
            .unwrap()["goal"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert!(
        adv.contains(&finale_goal),
        "the description must default to the finale quest's goal (`{finale_goal}`):\n{adv}"
    );
}

/// An authored `world.outro` wins over the derived default — and, being a
/// `Campaign` field, is what a sidecar translates.
#[test]
fn authored_world_outro_is_used() {
    let mut world: serde_json::Value = serde_json::from_str(&read_hw("world.json")).unwrap();
    world["dsl_version"] = "0.6.0".into();
    world["content"]["outro"] = "The moor keeps its silence.".into();
    let out = build_ok(&parse_hw_with(&[(
        "world.json",
        serde_json::to_string(&world).unwrap(),
    )]));
    let adv = fn_body(&out, "advancement/campaign_complete.json");
    assert!(
        adv.contains("The moor keeps its silence."),
        "an authored world.outro must be the advancement description:\n{adv}"
    );
}

// --- mannequin pose ---------------------------------------------------------

/// An NPC whose `base_entity` is spelled `minecraft:mannequin` by hand takes the
/// unskinned summon path, which emitted no `pose` — the mannequin then serialized
/// its pose as `DYING` and the server failed to encode it at save time.
#[test]
fn hand_written_mannequin_base_entity_gets_a_pose() {
    let mut npcs: serde_json::Value = serde_json::from_str(&read_hw("npcs.json")).unwrap();
    npcs["content"]["npcs"][0]["base_entity"] = "minecraft:mannequin".into();
    let out = build_ok(&parse_hw_with(&[(
        "npcs.json",
        serde_json::to_string(&npcs).unwrap(),
    )]));
    let setup = fn_body(&out, "function/setup_finish.mcfunction");
    let summon = setup
        .lines()
        .find(|l| l.starts_with("summon minecraft:mannequin"))
        .unwrap_or_else(|| panic!("no mannequin summon in:\n{setup}"));
    assert!(
        summon.contains(r#"pose:"standing""#),
        "an unskinned mannequin summon must still carry a pose: {summon}"
    );
}

/// A non-mannequin entity gains no `pose` field — the guard is entity-conditional,
/// so every existing campaign stays byte-identical.
#[test]
fn non_mannequin_summon_is_unchanged() {
    let out = build_ok(&parse_hw_with(&[]));
    let setup = fn_body(&out, "function/setup_finish.mcfunction");
    let summon = setup
        .lines()
        .find(|l| l.starts_with("summon minecraft:villager"))
        .unwrap_or_else(|| panic!("no villager summon in:\n{setup}"));
    assert!(
        !summon.contains("pose:"),
        "a villager must not gain a mannequin pose: {summon}"
    );
}
