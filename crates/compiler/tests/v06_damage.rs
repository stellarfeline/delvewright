//! DSL v0.6 `damage-players` emission + PackTest: the `/damage` primitive the
//! effect lowers to, the default + explicit damage type, the `in` box filter, and
//! the generated PackTest that drives the damage on a dummy and asserts it lands.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

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
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn build(campaign: &Campaign, prefabs: &PrefabRegistry) -> BuildOutput {
    let plan = Plan::build(campaign, prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
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
    )
    .expect("every emitted command validates (mecha-equivalent structure check)")
}

fn shipped_functions(out: &BuildOutput) -> String {
    let mut s = String::new();
    for (path, bytes) in out {
        if path.starts_with("datapack/") && path.ends_with(".mcfunction") {
            s.push_str(std::str::from_utf8(bytes).unwrap());
            s.push('\n');
        }
    }
    s
}

fn build_hw(on_complete: &str) -> BuildOutput {
    let c = parse_hw(&quests_doc(on_complete));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    build(&c, &prefabs)
}

/// An unscoped `damage-players` on a quest beat lowers to `damage @a[…]
/// <amount> minecraft:generic` — the hazard is a fact about the delve, so it hits
/// the whole party (spec-0018) — behind the cutscene guard folded into the
/// selector (a player watching a cinematic is never harmed).
#[test]
fn damage_players_default_emits_generic_at_the_party() {
    let out = build_hw(
        r#"{ "type": "damage-players", "amount": 6 },
           { "type": "campaign-complete" }"#,
    );
    let all = shipped_functions(&out);
    assert!(
        all.lines()
            .any(|l| l == "execute as @a[tag=!dw_cutscene] run damage @s 6 minecraft:generic"),
        "expected the cutscene-guarded party damage; functions:\n{all}"
    );
}

/// An explicit `damage_type` maps to its curated vanilla id.
#[test]
fn damage_players_explicit_type_maps_to_vanilla_id() {
    let out = build_hw(
        r#"{ "type": "damage-players", "amount": 40, "damage_type": "wither" },
           { "type": "campaign-complete" }"#,
    );
    let all = shipped_functions(&out);
    assert!(
        all.lines()
            .any(|l| l == "execute as @a[tag=!dw_cutscene] run damage @s 40 minecraft:wither"),
        "expected the cutscene-guarded party wither damage; functions:\n{all}"
    );
}

/// An `in` box filters to party members inside the anchor-centred AABB, folded
/// into the target selector so each player is judged on their own position.
#[test]
fn damage_players_in_box_filters_the_party_by_position() {
    let out = build_hw(
        r#"{ "type": "damage-players", "amount": 6,
             "in": { "anchor": "anchor/exit", "extent": [3, 2, 3] } },
           { "type": "campaign-complete" }"#,
    );
    let all = shipped_functions(&out);
    let guarded = all.lines().any(|l| {
        l.starts_with("execute as @a[x=")
            && l.contains("dx=6")
            && l.contains("dy=4")
            && l.contains("dz=6")
            && l.contains("tag=!dw_cutscene")
            && l.ends_with("] run damage @s 6 minecraft:generic")
    });
    assert!(
        guarded,
        "expected a box-filtered `execute as @a[box] run damage @s 6 …`:\n{all}"
    );
}

/// The generated PackTest drives the damage on a summoned dummy and asserts its
/// Health strictly dropped — the runtime proof that `damage-players` lands.
#[test]
fn damage_players_emits_packtest_asserting_damage() {
    let out = build_hw(
        r#"{ "type": "damage-players", "amount": 12, "damage_type": "magic" },
           { "type": "campaign-complete" }"#,
    );
    let path = "packtest-datapack/data/hello-world/test/v06_damage.mcfunction";
    let body = std::str::from_utf8(out.get(path).expect("v06_damage packtest emitted")).unwrap();
    assert!(
        body.contains("summon minecraft:zombie") && body.contains("dw_dmgtest"),
        "packtest must summon a tagged dummy:\n{body}"
    );
    assert!(
        body.contains("damage @e[tag=dw_dmgtest,limit=1] 12 minecraft:magic"),
        "packtest must apply the declared amount + type to the dummy:\n{body}"
    );
    assert!(
        body.contains("assert score #drop_dmg dw.sys matches 1.."),
        "packtest must assert the dummy's health dropped:\n{body}"
    );
    // Batch model: the tag is cleared on entry (never assume a fresh world) and
    // again on exit (no residue for a sibling).
    assert!(
        body.matches("kill @e[tag=dw_dmgtest]").count() >= 2,
        "packtest clears its dummy tag on entry and exit:\n{body}"
    );
}
