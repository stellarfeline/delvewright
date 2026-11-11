//! spec-0134 §1 — a datum counts bodies in a box, and a trigger fires when one
//! enters. Proven on the souls-bonfire fixture: `state/sheep-home` counts the
//! `wave/ambush` sheep in a box at the wave's own anchor; `trigger/sheep-in`
//! fires when the first one is counted; the shrine's approach is gated on two.
//!
//! What the document tier refuses is `DW1088` in its three shapes; what the
//! emitter writes is the `census_tick` store line and the edge; what the nav
//! proof asks (`DW1089`) is `nav::counted`'s unit tests over a fenced pen.

use crate::common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Diagnostic, RawCampaign, parse_campaign, validate_campaign};
use serde_json::{Value, json};

const NS: &str = "souls-bonfire";

fn fixture() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join("souls-bonfire")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(fixture().join(name)).unwrap()
}

/// The fixture's quests with the census, its trigger and its gate, then
/// `mutate`.
fn quests(mutate: impl FnOnce(&mut Value)) -> Value {
    let mut q: Value = serde_json::from_str(&read("quests.json")).unwrap();
    let c = &mut q["content"];
    c["state"] = json!([{
        "id": "state/sheep-home", "scope": "party",
        "counts": {"region": {"anchor": "anchor/wave", "extent": [2, 1, 2]},
                   "of": {"wave": "wave/ambush"}}
    }]);
    c["triggers"].as_array_mut().unwrap().push(json!({
        "id": "trigger/sheep-in",
        "on": {"on": "body-enters", "state": "state/sheep-home"},
        "effects": [
            {"type": "narrate", "text": "Something woolly is home.", "style": "chat"}
        ]
    }));
    c["triggers"][0]["requires_state"] =
        json!([{"state": "state/sheep-home", "op": "at-least", "value": 2}]);
    mutate(&mut q);
    q
}

fn campaign(q: &Value) -> delvewright_dsl::Campaign {
    parse_campaign(&RawCampaign {
        world: read("world.json"),
        npcs: read("npcs.json"),
        classes: read("classes.json"),
        quest_plan: read("quest-plan.json"),
        quests: q.to_string(),
        dialogue: read("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("parses")
}

fn diags(q: &Value) -> Vec<Diagnostic> {
    validate_campaign(&campaign(q))
}

fn codes(d: &[Diagnostic], code: &str) -> Vec<String> {
    d.iter()
        .filter(|x| x.code == code)
        .map(|x| x.path.clone())
        .collect()
}

fn build(c: &delvewright_dsl::Campaign) -> BuildOutput {
    let loaded = load_campaign_dir(&fixture()).unwrap();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
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
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("emission succeeds")
}

fn function(out: &BuildOutput, name: &str) -> Option<String> {
    out.get(&format!("datapack/data/{NS}/function/{name}.mcfunction"))
        .map(|b| String::from_utf8(b.clone()).unwrap())
}

#[test]
fn a_census_and_its_trigger_validate() {
    let d = diags(&quests(|_| {}));
    assert!(codes(&d, "DW1088").is_empty(), "{d:#?}");
    assert!(codes(&d, "DW0502").is_empty(), "the gate reads it: {d:#?}");
    assert!(
        codes(&d, "DW0501").is_empty(),
        "the census writes it: {d:#?}"
    );
}

/// The perturbation the spec names: an author's write of the counted datum.
#[test]
fn an_authors_write_of_a_counted_datum_is_dw1088() {
    let q = quests(|q| {
        q["content"]["triggers"][0]["effects"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "add-state", "state": "state/sheep-home", "amount": 1}));
    });
    assert_eq!(
        codes(&diags(&q), "DW1088"),
        vec!["/content/triggers/0/effects/1/state"]
    );
}

#[test]
fn a_player_scoped_census_is_dw1088() {
    let q = quests(|q| q["content"]["state"][0]["scope"] = json!("player"));
    assert_eq!(codes(&diags(&q), "DW1088"), vec!["/content/state/0/scope"]);
}

#[test]
fn a_body_enters_on_a_datum_that_counts_nothing_is_dw1088() {
    let q = quests(|q| {
        q["content"]["state"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": "state/plain", "scope": "party"}));
        let t = q["content"]["triggers"].as_array_mut().unwrap();
        t.last_mut().unwrap()["on"]["state"] = json!("state/plain");
    });
    let d = diags(&q);
    assert!(
        codes(&d, "DW1088").contains(&"/content/triggers/2/on/state".to_string()),
        "{d:#?}"
    );
}

#[test]
fn a_census_of_an_undeclared_wave_is_dw0112() {
    let q = quests(|q| {
        q["content"]["state"][0]["counts"]["of"] = json!({"wave": "wave/nobody"});
    });
    assert_eq!(
        codes(&diags(&q), "DW0112"),
        vec!["/content/state/0/counts/of/wave"]
    );
}

#[test]
fn the_census_tick_writes_the_count_and_offers_the_edge() {
    let out = build(&campaign(&quests(|_| {})));
    let tick = function(&out, "census_tick").expect("a census emits its tick");
    let lines: Vec<&str> = tick.lines().collect();
    let store = lines[0];
    assert!(
        store.starts_with(
            "execute store result score #party dw.s_sheep_home if entity @e[tag=dw_wave_ambush,x="
        ) && store.ends_with(",tag=!dw_fixture]"),
        "{tick}"
    );
    // The box: the wave anchor ± [2, 1, 2] — dx = dz = 4, dy = 2.
    assert!(store.contains(",dx=4,") && store.contains(",dy=2,") && store.contains(",dz=4,"));
    // The edge: fires at 1.., latched, re-armed at 0.
    assert!(
        lines.iter().any(|l| l.contains("unless score #be_sheep_in dw.sys matches 1 if score #party dw.s_sheep_home matches 1.. run function souls-bonfire:be_sheep_in")),
        "{tick}"
    );
    assert!(
        lines.contains(&"execute if score #party dw.s_sheep_home matches ..0 run scoreboard players set #be_sheep_in dw.sys 0"),
        "{tick}"
    );
    assert_eq!(
        *lines.last().unwrap(),
        "schedule function souls-bonfire:census_tick 10t replace"
    );
    let be = function(&out, "be_sheep_in").expect("the dispatch");
    assert_eq!(
        be.lines().collect::<Vec<_>>(),
        vec![
            "scoreboard players set #be_sheep_in dw.sys 1",
            "function souls-bonfire:trig_sheep_in"
        ]
    );
    // `once` (the default) guards the edge with the trigger's own sentinel.
    assert!(tick.contains(
        "execute unless score #trig_sheep_in dw.sys matches 1 unless score #be_sheep_in"
    ));
    // Started once, in setup.
    let all: String = out
        .iter()
        .filter(|(k, _)| k.ends_with(".mcfunction"))
        .map(|(_, v)| String::from_utf8_lossy(v).to_string())
        .collect();
    assert_eq!(
        all.matches("schedule function souls-bonfire:census_tick 10t replace")
            .count(),
        2,
        "once in setup, once in the tick itself"
    );
    // A gate on the datum reads it as any datum's.
    assert!(
        all.contains("if score #party dw.s_sheep_home matches 2.."),
        "the shrine's gate"
    );
}

#[test]
fn a_repeatable_body_enters_re_arms_at_zero_and_has_no_once_guard() {
    let q = quests(|q| {
        let t = q["content"]["triggers"].as_array_mut().unwrap();
        t.last_mut().unwrap()["once"] = json!(false);
    });
    let tick = function(&build(&campaign(&q)), "census_tick").unwrap();
    assert!(!tick.contains("#trig_sheep_in"), "{tick}");
    assert!(tick.contains("matches ..0 run scoreboard players set #be_sheep_in dw.sys 0"));
}

#[test]
fn no_census_emits_no_tick() {
    let loaded = load_campaign_dir(&fixture()).unwrap();
    let c = parse_campaign(&loaded.raw).unwrap();
    let out = build(&c);
    assert!(function(&out, "census_tick").is_none());
    let all: String = out
        .values()
        .map(|v| String::from_utf8_lossy(v).to_string())
        .collect();
    assert!(!all.contains("census_tick"));
}
