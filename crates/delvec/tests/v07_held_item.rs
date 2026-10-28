//! `interact.requires_item` is a HELD gate, and `answers.missing_item` is the
//! diegetic answer to a press that arrives without the item in hand
//! (spec-0117 §4).
//!
//! Three things are proven here that no other tier can prove:
//! - the completion gate reads `weapon.mainhand`, never the inventory;
//! - `collect`'s hold check still reads `container.*` — it counts an inventory,
//!   which is a different question and must not be dragged along by the fix;
//! - the empty-hand answer dispatches exactly where the completion does not:
//!   open, and the hand lacks the item.
//!
//! The generated `verb_interact_held` PackTest carries the live half (carried ≠
//! held on a real server); the narration itself has no game state to assert
//! against, so its exact emitted line is asserted here instead.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, DSL_VERSION, RawCampaign, parse_campaign};

const ITEM: &str = "minecraft:tripwire_hook";
const HINT: &str = "The bar does not shift for bare hands.";

/// A hello-world `quests` doc whose second objective is a gated `interact`,
/// answering in each of its states.
fn quests_doc() -> String {
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
          {{ "type": "interact", "id": "obj/pry", "anchor": "anchor/exit", "prop": {{ "block": "minecraft:torch" }},
             "requires_item": "{ITEM}", "after": ["obj/talk"],
             "answers": {{ "not_yet": "Not yet.", "missing_item": "{HINT}", "done": "Pried." }} }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
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
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn build(campaign: &Campaign) -> BuildOutput {
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
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &tree,
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

fn file(out: &BuildOutput, suffix: &str) -> String {
    let (_, bytes) = out
        .iter()
        .find(|(p, _)| p.ends_with(suffix))
        .unwrap_or_else(|| panic!("no emitted file ends with `{suffix}`"));
    String::from_utf8(bytes.clone()).unwrap()
}

/// The completion gate reads the MAIN HAND. Presenting the item is the action;
/// carrying it in a backpack is not, and the pre-ruling `container.*` reading let
/// a player blind a sleeping giant with the stake still stowed.
#[test]
fn the_interact_gate_reads_the_main_hand() {
    let tick = file(
        &build(&parse_hw(&quests_doc())),
        "/function/tick.mcfunction",
    );
    let gate = tick
        .lines()
        .find(|l| l.contains("complete_o_pry"))
        .expect("the interact completion line is emitted");
    assert!(
        gate.contains(&format!("if items entity @s weapon.mainhand {ITEM}")),
        "the gate must read the main hand, got: {gate}"
    );
    assert!(
        !gate.contains("container."),
        "the gate must not read the inventory any more, got: {gate}"
    );
}

/// `collect`'s hold check is deliberately NOT changed: it counts how many of an
/// item a player carries, which is an inventory question. The fix must not bleed
/// into it.
#[test]
fn collect_still_counts_the_whole_inventory() {
    let quests = read_hw("quests.json");
    // hello-world has no `collect`; the v0.4 showcase fixture does.
    let showcase =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v04-showcase");
    let raw = RawCampaign {
        world: std::fs::read_to_string(showcase.join("world.json")).unwrap(),
        npcs: std::fs::read_to_string(showcase.join("npcs.json")).unwrap(),
        classes: std::fs::read_to_string(showcase.join("classes.json")).unwrap(),
        quest_plan: std::fs::read_to_string(showcase.join("quest-plan.json")).unwrap(),
        quests: std::fs::read_to_string(showcase.join("quests.json")).unwrap(),
        dialogue: std::fs::read_to_string(showcase.join("dialogue.json")).unwrap(),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    assert!(!quests.is_empty());
    let c = parse_campaign(&raw).expect("showcase parses");
    let tick = file(&build(&c), "/function/tick.mcfunction");
    assert!(
        tick.contains("store result score @s dw.hold if items entity @s container.*"),
        "the collect hold count must still read the whole inventory:\n{tick}"
    );
}

/// The empty-hand answer: a press dispatch guarded by the objective's open
/// state and the negation of the completion's main-hand condition, so a press
/// either completes the objective or is answered, never both.
#[test]
fn the_missing_item_answer_dispatches_on_the_empty_hand() {
    let out = build(&parse_hw(&quests_doc()));
    let press = file(
        &out,
        "/function/press_dw_answer_obj_pry_missing_item.mcfunction",
    );
    let dispatch = press
        .lines()
        .find(|l| l.contains("run function") && l.contains("trig_dw_answer_obj_pry_missing_item"))
        .unwrap_or_else(|| panic!("the empty-hand answer is dispatched:\n{press}"));
    assert!(
        dispatch.contains("if score #pst_obj_pry dw.sys matches 1 ")
            && dispatch.contains(&format!("unless items entity @s weapon.mainhand {ITEM} ")),
        "open, and the hand empty of the item: {dispatch}"
    );
    let answer = file(
        &out,
        "/function/trig_dw_answer_obj_pry_missing_item.mcfunction",
    );
    assert!(
        answer.contains("title @s actionbar") && answer.contains(HINT),
        "the line goes to the presser's actionbar:\n{answer}"
    );
    assert!(
        answer.contains("scoreboard players set #ans_obj_pry dw.sys 2"),
        "the trace records the open-state answer:\n{answer}"
    );
    let tick = file(&out, "/function/tick.mcfunction");
    let complete = tick
        .lines()
        .find(|l| l.contains("complete_o_pry"))
        .expect("completion line");
    assert!(
        complete.contains(&format!("if items entity @s weapon.mainhand {ITEM} ")),
        "the completion is the answer's complement: {complete}"
    );
}

/// The generated PackTest proves the semantics live: carried-but-not-held does not
/// complete (and the item really IS carried), held does.
#[test]
fn the_generated_packtest_proves_carried_is_not_held() {
    let out = build(&parse_hw(&quests_doc()));
    let t = file(&out, "test/verb_interact_held.mcfunction");
    assert!(t.contains("# @dummy"), "own dummy: {t}");
    assert!(
        t.contains(&format!(
            "item replace entity @a[tag=dw_t_vheld,limit=1] inventory.0 with {ITEM}"
        )),
        "phase A must put the item in the pack:\n{t}"
    );
    assert!(
        t.contains(
            "item replace entity @a[tag=dw_t_vheld,limit=1] weapon.mainhand with minecraft:air"
        ),
        "phase A must empty the hand:\n{t}"
    );
    assert!(
        t.contains("assert score #carried_vheld dw.sys matches 1"),
        "phase A must prove the item really is carried, else the test is vacuous:\n{t}"
    );
    let obj_assert_zero = t
        .lines()
        .position(|l| l.starts_with("assert score #party dw.o_pry matches 0"))
        .expect("phase A asserts non-completion");
    let obj_assert_one = t
        .lines()
        .position(|l| l.starts_with("assert score #party dw.o_pry matches 1"))
        .expect("phase B asserts completion");
    assert!(
        obj_assert_zero < obj_assert_one,
        "phases must run carried-then-held:\n{t}"
    );
    assert!(
        t.contains(&format!(
            "item replace entity @a[tag=dw_t_vheld,limit=1] weapon.mainhand with {ITEM}"
        )),
        "phase B must present the item:\n{t}"
    );
}
