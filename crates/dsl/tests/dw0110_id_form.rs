//! `DW0110` names the form of the id type it rejected.
//!
//! # The gap this closes
//!
//! One macro in `validate::syntax` is the single path every id type's syntax
//! refusal goes through, and it used to answer all of them with the same
//! sentence and the same three examples — `area/keep`, `npc/keeper`,
//! `quest/find-key`. A dialogue node id written `dlg/<npc>/<name>` was refused
//! by a message that never spelled `dlg/<kebab>`, and the rule it needed lived
//! only in the schema description; the same held for every id type that is not
//! one of those three.
//!
//! The prefix is a property of the id TYPE (`Id::PREFIX`), so the answer is
//! derived from the type at every site — see `ids::syntax_form`. The
//! per-section refusals that hand-write their own prefix (`wave/`, `trap/`,
//! `loot/`, …) are that same fact copied by hand, which is exactly why the
//! general path never had it.
//!
//! So this file asserts over the **object class**, not over the one instance
//! that was reported: several id types from four different stage documents,
//! each of which must see its own form and no other type's.

mod common;

use delvewright_dsl::{Diagnostic, RawCampaign, check_campaign};
use serde_json::{Value, json};

fn dw0110(raw: RawCampaign) -> Vec<Diagnostic> {
    check_campaign(&raw)
        .into_iter()
        .filter(|d| d.code == "DW0110")
        .collect()
}

/// The one `DW0110` a one-field perturbation raises, and its message.
fn only_message(raw: RawCampaign) -> String {
    let got = dw0110(raw);
    assert_eq!(got.len(), 1, "expected exactly one DW0110, got: {got:#?}");
    got.into_iter().next().unwrap().message
}

fn patched(stage: &str, f: impl FnOnce(&mut Value)) -> RawCampaign {
    let text = common::patch_doc(&common::read_valid(&format!("{stage}.json")), f);
    let mut raw = common::valid_raw();
    match stage {
        "world" => raw.world = text,
        "npcs" => raw.npcs = text,
        "classes" => raw.classes = text,
        "quest-plan" => raw.quest_plan = text,
        "quests" => raw.quests = text,
        "dialogue" => raw.dialogue = text,
        other => panic!("unknown stage `{other}`"),
    }
    raw
}

/// Every id type reached below states ITS OWN form, and states it in the shape
/// an author can copy.
///
/// The second half of each assertion is the one that would have caught the
/// original defect: a message that names the general rule and three examples of
/// other types passes "mentions kebab-case" and fails this.
fn assert_names_own_form(message: &str, prefix: &str) {
    assert!(
        message.contains(&format!("`{prefix}/<kebab>`")),
        "a DW0110 over a `{prefix}/…` id must spell `{prefix}/<kebab>`: {message}"
    );
    for other in ["area", "npc", "quest", "dlg", "obj", "class"] {
        if other == prefix {
            continue;
        }
        assert!(
            !message.contains(&format!("`{other}/<kebab>`")),
            "a DW0110 over a `{prefix}/…` id must not offer `{other}/<kebab>` as the form: \
             {message}"
        );
    }
}

/// The instance the authoring walk reported: a dialogue node id with a second
/// segment. `dlg/<kebab>` is exactly one segment after the prefix, and until
/// now the refusal never said so.
#[test]
fn a_dialogue_node_id_is_told_the_dlg_form() {
    let raw = patched("dialogue", |v| {
        v["content"]["dialogues"][0]["nodes"][0]["id"] = json!("dlg/keeper/greeting");
        v["content"]["dialogues"][0]["root"] = json!("dlg/keeper/greeting");
    });
    assert_names_own_form(&only_message(raw), "dlg");
}

/// An objective id, from the stage-5 document.
#[test]
fn an_objective_id_is_told_the_obj_form() {
    let raw = patched("quests", |v| {
        v["content"]["quests"][0]["objectives"][0]["id"] = json!("obj/Talk");
    });
    let m = only_message(raw);
    assert_names_own_form(&m, "obj");
    assert!(m.contains("no capitals"), "{m}");
}

/// An NPC id, from stage 2.
#[test]
fn an_npc_id_is_told_the_npc_form() {
    let raw = patched("npcs", |v| {
        v["content"]["npcs"][0]["id"] = json!("keeper");
    });
    assert_names_own_form(&only_message(raw), "npc");
}

/// A class id, from stage 3 — a type none of the three old examples covered.
#[test]
fn a_class_id_is_told_the_class_form() {
    let raw = patched("classes", |v| {
        v["content"]["classes"][0]["id"] = json!("class/Heavy_Guard");
    });
    assert_names_own_form(&only_message(raw), "class");
}

/// An area id, from stage 1 — one of the three types the old message happened
/// to name, kept here so the change is not only tested where it was new.
#[test]
fn an_area_id_is_told_the_area_form() {
    let raw = patched("world", |v| {
        v["content"]["areas"][0]["id"] = json!("area/The Keep");
    });
    assert_names_own_form(&only_message(raw), "area");
}

/// The valid campaign raises no `DW0110`, so none of the above is green because
/// the check stopped binding.
#[test]
fn the_valid_campaign_raises_none() {
    assert!(dw0110(common::valid_raw()).is_empty());
}
