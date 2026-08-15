//! `interact.requires_item` is a HELD gate, and
//! `missing_item_hint` is the diegetic answer to a click that arrives without the
//! item in hand.
//!
//! Three things are proven here that no other tier can prove:
//! - the completion gate reads `weapon.mainhand`, never the inventory;
//! - `collect`'s hold check still reads `container.*` — it counts an inventory,
//!   which is a different question and must not be dragged along by the fix;
//! - the hint is one extra guarded `tellraw` and nothing else, so an objective
//!   that does not author it emits byte-for-byte what it emitted before the
//!   field existed.
//!
//! The generated `verb_interact_held` PackTest carries the live half (carried ≠
//! held on a real server); the narration itself has no game state to assert
//! against, so its exact emitted line is asserted here instead.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

const ITEM: &str = "minecraft:tripwire_hook";
const HINT: &str = "The bar does not shift for bare hands.";

/// A hello-world `quests` doc whose second objective is a gated `interact`.
/// `extra` splices the objective's optional fields.
fn quests_doc(extra: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.7.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "interact", "id": "obj/pry", "anchor": "anchor/exit",
             "requires_item": "{ITEM}", "after": ["obj/talk"]{extra} }}
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
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn build(campaign: &Campaign) -> BuildOutput {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
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
        &prefabs,
        None,
        "unpinned",
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
        &build(&parse_hw(&quests_doc(""))),
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
    };
    assert!(!quests.is_empty());
    let c = parse_campaign(&raw).expect("showcase parses");
    let tick = file(&build(&c), "/function/tick.mcfunction");
    assert!(
        tick.contains("store result score @s dw.hold if items entity @s container.*"),
        "the collect hold count must still read the whole inventory:\n{tick}"
    );
}

/// The empty-hand line: one guarded, per-player `tellraw`, carrying the same
/// activation guard as the completion line it shadows — so an inactive or already
/// finished objective stays silent — and negating the same main-hand condition.
#[test]
fn the_missing_item_hint_narrates_the_empty_hand() {
    let doc = quests_doc(&format!(r#", "missing_item_hint": "{HINT}""#));
    let tick = file(&build(&parse_hw(&doc)), "/function/tick.mcfunction");
    let hint = tick
        .lines()
        .find(|l| l.contains("tellraw") && l.contains(HINT))
        .unwrap_or_else(|| panic!("the missing-item hint must be emitted:\n{tick}"));
    assert!(
        hint.contains("unless items entity @s weapon.mainhand ") && hint.contains(ITEM),
        "the hint fires exactly when the hand is empty of the item, got: {hint}"
    );
    assert!(hint.contains("run tellraw @s "), "per-player: {hint}");

    // Same activation guard as the completion line — nothing more, nothing less.
    let complete = tick
        .lines()
        .find(|l| l.contains("complete_o_pry"))
        .expect("completion line");
    let guard_of = |l: &str| {
        let start = l.find("] if score").expect("guard start") + 1;
        let end = l
            .find(" if items")
            .or_else(|| l.find(" unless items"))
            .expect("item guard");
        l[start..end].to_string()
    };
    assert_eq!(
        guard_of(hint),
        guard_of(complete),
        "the hint must be gated exactly like the affordance it explains"
    );

    // It must sit BEFORE the trigger reset that consumes the click, so one click
    // yields exactly one line.
    let idx = |needle: &str| tick.lines().position(|l| l.contains(needle)).unwrap();
    assert!(
        idx(HINT) < idx("scoreboard players reset @s dw.i_pry"),
        "the hint must run before the click record is consumed"
    );
}

/// An objective that does not author the field emits exactly what it emitted
/// before the field existed: the hint costs one line and only when authored.
#[test]
fn an_unauthored_hint_costs_nothing() {
    let without = file(
        &build(&parse_hw(&quests_doc(""))),
        "/function/tick.mcfunction",
    );
    assert!(
        !without.contains("tellraw @s") || !without.contains("weapon.mainhand\"}"),
        "no hint may be emitted without the field"
    );
    let with = file(
        &build(&parse_hw(&quests_doc(&format!(
            r#", "missing_item_hint": "{HINT}""#
        )))),
        "/function/tick.mcfunction",
    );
    let added: Vec<&str> = with
        .lines()
        .filter(|l| !without.lines().any(|w| w == *l))
        .collect();
    assert_eq!(
        added.len(),
        1,
        "authoring the hint must add exactly one command, got: {added:?}"
    );
    assert!(added[0].contains(HINT));
}

/// The generated PackTest proves the semantics live: carried-but-not-held does not
/// complete (and the item really IS carried), held does.
#[test]
fn the_generated_packtest_proves_carried_is_not_held() {
    let out = build(&parse_hw(&quests_doc("")));
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
