//! spec-0095: the party is seen in its own cutscenes.
//!
//! A cutscene declared `present` (the default) places a stand-in for every
//! player in play before the party goes to spectator and removes it, unseen, at
//! its end; one declared `absent` places none and is a function of its own.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, DSL_VERSION, RawCampaign, parse_campaign};

const NS: &str = "hello-world";

fn quests_doc(cutscene: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
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
        "on_complete": [ {cutscene}, {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn build(cutscene: &str) -> BuildOutput {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests_doc(cutscene),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    let campaign: &'static Campaign =
        Box::leak(Box::new(parse_campaign(&raw).expect("campaign parses")));
    let prefabs: &'static PrefabRegistry = Box::leak(Box::new(
        PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap(),
    ));
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
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

const SHOT: &str = r#""seconds": 2,
             "path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
                       { "anchor": "anchor/exit", "offset": [2, 2, 0] } ]"#;

fn function(out: &BuildOutput, name: &str) -> Option<String> {
    out.get(&format!("datapack/data/{NS}/function/{name}.mcfunction"))
        .map(|b| String::from_utf8(b.clone()).unwrap())
}

fn named(out: &BuildOutput, prefix: &str) -> Vec<String> {
    let p = format!("datapack/data/{NS}/function/{prefix}");
    out.keys()
        .filter(|k| k.starts_with(&p))
        .map(|k| {
            k.rsplit_once("/function/")
                .unwrap()
                .1
                .trim_end_matches(".mcfunction")
                .to_string()
        })
        .collect()
}

/// The default: stand-ins placed in the start, before spectator, by the shared
/// function, and removed at the end by the unseen exit.
#[test]
fn a_cutscene_shows_the_party_by_default() {
    let out = build(&format!(r#"{{ "type": "cutscene", {SHOT} }}"#));
    let start_name = "cs_exit_2_2";
    let start = function(&out, start_name).expect("the start function");
    let lines: Vec<&str> = start.lines().collect();
    let placed = lines
        .iter()
        .position(|l| {
            *l == format!(
                "execute as @a[tag=!dw_cutscene,gamemode=!spectator] at @s run function {NS}:cs_standin"
            )
        })
        .expect("the start places a stand-in for every player in play");
    let spectator = lines
        .iter()
        .position(|l| *l == "gamemode spectator @a")
        .unwrap();
    assert!(
        placed < spectator,
        "placed from the body, before spectator:\n{start}"
    );
    assert!(start.contains("add dw_standin_exit_2_2"), "{start}");
    let end = function(&out, "cs_end_exit_2_2").unwrap();
    assert!(
        end.contains("execute as @e[tag=dw_standin_exit_2_2] at @s run tp @s ~ -128 ~"),
        "the end removes them unseen:\n{end}"
    );
    let standin = function(&out, "cs_standin").expect("the shared stand-in function");
    assert!(
        standin.starts_with("summon minecraft:mannequin ~ ~ ~ "),
        "{standin}"
    );
    assert!(
        standin.contains(&format!("loot {NS}:standin_profile")),
        "{standin}"
    );
    let loot = out
        .get(&format!(
            "datapack/data/{NS}/loot_table/standin_profile.json"
        ))
        .expect("the profile loot table");
    let loot = String::from_utf8(loot.clone()).unwrap();
    assert!(loot.contains("minecraft:fill_player_head"), "{loot}");
    assert!(out.contains_key(&format!(
        "packtest-datapack/data/{NS}/test/standin.mcfunction"
    )));
    let gate = String::from_utf8(out["validation/stand-in-gate.json"].clone()).unwrap();
    assert!(
        gate.contains("\"present\": 1") && gate.contains("\"absent\": 0"),
        "{gate}"
    );
}

/// `party: absent` places nothing, is its own function, and ships no stand-in
/// machinery at all when it is the campaign's only cutscene.
#[test]
fn an_absent_party_places_no_stand_in() {
    let out = build(&format!(
        r#"{{ "type": "cutscene", "party": "absent", {SHOT} }}"#
    ));
    assert_eq!(
        named(&out, "cs_exit"),
        vec!["cs_exit_2_2_absent".to_string()]
    );
    let start = function(&out, "cs_exit_2_2_absent").unwrap();
    assert!(!start.contains("cs_standin"), "{start}");
    let end = function(&out, "cs_end_exit_2_2_absent").unwrap();
    assert!(!end.contains("dw_standin"), "{end}");
    assert!(function(&out, "cs_standin").is_none());
    assert!(!out.contains_key(&format!(
        "datapack/data/{NS}/loot_table/standin_profile.json"
    )));
    assert!(!out.contains_key(&format!(
        "packtest-datapack/data/{NS}/test/standin.mcfunction"
    )));
    let gate = String::from_utf8(out["validation/stand-in-gate.json"].clone()).unwrap();
    assert!(
        gate.contains("\"present\": 0") && gate.contains("\"absent\": 1"),
        "{gate}"
    );
}

/// Writing the default is the default: `party: present` is byte-identical to
/// leaving it out.
#[test]
fn present_is_the_default_spelled() {
    let a = build(&format!(r#"{{ "type": "cutscene", {SHOT} }}"#));
    let b = build(&format!(
        r#"{{ "type": "cutscene", "party": "present", {SHOT} }}"#
    ));
    let datapack = |o: &BuildOutput| {
        o.iter()
            .filter(|(k, _)| k.starts_with("datapack/"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(datapack(&a), datapack(&b));
}
