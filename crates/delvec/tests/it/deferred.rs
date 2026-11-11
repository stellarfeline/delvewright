//! spec-0134 §6 — a fill waits for its beat. `loot[].filled: "by-effect"`
//! moves a fill out of `setup_finish` into its own `fill_<loot>` function,
//! which the `fill-loot` verb calls (emission is proven in `emit::loot`'s
//! unit tests, `a_fill_that_waits_is_its_own_function`); here, the document
//! tier's `DW1093` both ways and `DW0112` for a fill nobody declares.
//!
//! The world-edits half of §6 (`applied: "by-effect"`, `apply-edits`,
//! the batch code) is not implemented in this round: a recorded debt.

use crate::common;

use delvewright_dsl::{DSL_VERSION, Diagnostic, RawCampaign, parse_campaign, validate_campaign};
use serde_json::{Value, json};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn diags(filled: Option<&str>, fill_loot: Option<&str>) -> Vec<Diagnostic> {
    let mut q: Value = serde_json::from_str(&read_hw("quests.json")).unwrap();
    q["dsl_version"] = json!(DSL_VERSION);
    let mut l = json!({"id": "loot/supplies", "anchor": "anchor/keeper-stand",
                       "items": [{"item": "minecraft:bread", "count": 3}]});
    if let Some(f) = filled {
        l["filled"] = json!(f);
    }
    q["content"]["loot"] = json!([l]);
    if let Some(id) = fill_loot {
        q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "fill-loot", "loot": id,
                         "happening": {"text": "the supplies are laid out", "verb": "learns"}}));
    }
    let c = parse_campaign(&RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: q.to_string(),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("parses");
    validate_campaign(&c)
}

fn codes(d: &[Diagnostic], code: &str) -> Vec<String> {
    d.iter()
        .filter(|x| x.code == code)
        .map(|x| x.path.clone())
        .collect()
}

#[test]
fn a_fill_that_waits_and_its_beat_agree() {
    let d = diags(Some("by-effect"), Some("loot/supplies"));
    assert!(codes(&d, "DW1093").is_empty(), "{d:#?}");
    assert!(codes(&d, "DW0112").is_empty(), "{d:#?}");
}

#[test]
fn a_fill_nothing_fires_is_dw1093() {
    let d = diags(Some("by-effect"), None);
    assert_eq!(
        codes(&d, "DW1093"),
        vec!["/content/loot/0/filled"],
        "{d:#?}"
    );
}

#[test]
fn a_second_fill_of_a_filled_holder_is_dw1093() {
    let d = diags(None, Some("loot/supplies"));
    assert_eq!(
        codes(&d, "DW1093"),
        vec!["/content/quests/0/on_objective_complete/obj/talk/1/loot"],
        "{d:#?}"
    );
    let d = diags(Some("at-load"), Some("loot/supplies"));
    assert_eq!(codes(&d, "DW1093").len(), 1, "{d:#?}");
}

#[test]
fn a_fill_nobody_declares_is_dw0112() {
    let d = diags(Some("by-effect"), Some("loot/nowhere"));
    assert!(
        codes(&d, "DW0112")
            .contains(&"/content/quests/0/on_objective_complete/obj/talk/1/loot".to_string()),
        "{d:#?}"
    );
    assert_eq!(codes(&d, "DW1093"), vec!["/content/loot/0/filled"]);
}
