//! **A `drop-stake` is gated, and `validation/death-plan.json` says so.**
//!
//! `on_death` effects carry a `when` like every other effect, so "this death
//! forfeits this stake" is a CONDITIONAL promise. The plan used to hand the bot
//! tier a bare list of stake ids, and the bot then asserted every forfeit
//! unconditionally: the gallery declares two of its four `drop-stake` effects
//! behind `flag/hall-sealed`, which is set long before the death-loop stage runs,
//! and every run of that stage reported the engine's correct refusal to take those
//! two purses as "the death took the wrong amount for `stake/tokens`". A red about
//! the engine, produced by an artifact that had thrown the condition away.
//!
//! What this file proves is the property that stops it coming back: **the guard
//! the emitter writes and the terms the plan carries are one reading of one
//! declaration.** They are rendered from the same
//! [`delvec::compiler::plan::GateTerm`]s, and a test that only checked the plan's
//! shape would pass over a plan that agreed with nothing.
//!
//! It also proves the conjunction a nested effect owes: a gated `sequence` around
//! an ungated `drop-stake` promises the drop only while the sequence's gate is
//! open, and a descent that read the leaf's own gate alone would hand the bot an
//! unconditional promise the campaign never made.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::{LoadedCampaign, load_campaign_dir};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, QuestEffect, parse_campaign};

/// The fixture that declares a stake, a currency and a lethal volume.
const NS: &str = "economy";

fn load() -> LoadedCampaign {
    load_campaign_dir(&common::compiler_fixtures_dir().join(NS)).unwrap()
}

/// Parse the fixture and replace its `on_death` bundle with `effects`.
fn campaign(loaded: &LoadedCampaign, effects: &str) -> Campaign {
    let mut c = parse_campaign(&loaded.raw).expect("fixture parses");
    c.quests.content.on_death =
        serde_json::from_str::<Vec<QuestEffect>>(effects).expect("death beat parses");
    c
}

fn build(loaded: &LoadedCampaign, c: &Campaign) -> BuildOutput {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = common::validation_diagnostics(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(diags.is_empty(), "fixture must validate clean: {diags:#?}");
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
    .expect("every emitted command validates")
}

fn text<'a>(out: &'a BuildOutput, path: &str) -> &'a str {
    std::str::from_utf8(out.get(path).unwrap_or_else(|| panic!("{path} is emitted"))).unwrap()
}

fn death_plan(out: &BuildOutput) -> serde_json::Value {
    serde_json::from_slice(
        out.get("validation/death-plan.json")
            .expect("a campaign with a stake and a volume ships a death plan"),
    )
    .unwrap()
}

/// The `drops_stake` entry for `stake`, or a panic naming what the plan holds.
fn drop_for<'a>(plan: &'a serde_json::Value, stake: &str) -> &'a serde_json::Value {
    plan["on_death"]["drops_stake"]
        .as_array()
        .expect("drops_stake is an array")
        .iter()
        .find(|d| d["stake"] == stake)
        .unwrap_or_else(|| panic!("no drop for {stake} in {}", plan["on_death"]["drops_stake"]))
}

const GATED: &str = r#"[
  { "type": "drop-stake", "stake": "stake/embers",
    "when": { "requires_state": [ { "state": "state/embers", "op": "at-least", "value": 3 } ] } }
]"#;

const UNGATED: &str = r#"[ { "type": "drop-stake", "stake": "stake/embers" } ]"#;

const NESTED_UNDER_A_GATE: &str = r#"[
  { "type": "sequence",
    "when": { "requires_state": [ { "state": "state/embers", "op": "at-least", "value": 3 } ] },
    "steps": [ { "at_ticks": 0, "effects": [ { "type": "drop-stake", "stake": "stake/embers" } ] } ] }
]"#;

/// **The motivating scenario.** A gated `drop-stake` reaches the bot tier as a
/// gated one, spelled in the terms the server itself is asked.
#[test]
fn a_gated_drop_stake_carries_its_gate_to_the_bot_tier() {
    let loaded = load();
    let out = build(&loaded, &campaign(&loaded, GATED));
    let plan = death_plan(&out);
    let terms = &drop_for(&plan, "stake/embers")["gates"][0]["terms"];
    assert_eq!(terms.as_array().map(Vec::len), Some(1), "{terms}");
    assert_eq!(terms[0]["objective"], "dw.s_embers");
    // `state/embers` is declared `player`-scoped, so the acting player holds it.
    assert_eq!(terms[0]["holder"], "@s");
    assert_eq!(terms[0]["min"], 3);
    assert_eq!(terms[0]["max"], serde_json::Value::Null);
    assert_eq!(terms[0]["negate"], false);
}

/// The property, not the shape: what the plan says and what the datapack does are
/// the same reading. A plan that carried a gate nothing was emitted against would
/// be a second authority on when this death forfeits.
#[test]
fn the_plan_s_terms_and_the_emitted_guard_are_one_reading() {
    let loaded = load();
    let out = build(&loaded, &campaign(&loaded, GATED));
    let fire = text(
        &out,
        &format!("datapack/data/{NS}/function/on_death_fire.mcfunction"),
    );
    let plan = death_plan(&out);
    let t = &drop_for(&plan, "stake/embers")["gates"][0]["terms"][0];
    let clause = format!(
        "if score {} {} matches {}..",
        t["holder"].as_str().unwrap(),
        t["objective"].as_str().unwrap(),
        t["min"].as_i64().unwrap()
    );
    assert!(
        fire.contains(&clause),
        "the plan's term renders to `{clause}`, which the emitted guard does not contain:\n{fire}"
    );
}

/// The control, and the byte-identity half: an ungated drop is one alternative
/// with no terms, and the emitted line carries no `execute` guard at all.
#[test]
fn an_ungated_drop_is_one_alternative_with_no_terms() {
    let loaded = load();
    let out = build(&loaded, &campaign(&loaded, UNGATED));
    let plan = death_plan(&out);
    let gates = &drop_for(&plan, "stake/embers")["gates"];
    assert_eq!(gates.as_array().map(Vec::len), Some(1), "{gates}");
    assert_eq!(
        gates[0]["terms"].as_array().map(Vec::len),
        Some(0),
        "{gates}"
    );
    let fire = text(
        &out,
        &format!("datapack/data/{NS}/function/on_death_fire.mcfunction"),
    );
    assert!(
        fire.lines()
            .any(|l| l == format!("function {NS}:stk_drop_embers")),
        "an ungated drop is emitted verbatim:\n{fire}"
    );
}

/// A `drop-stake` inherits every enclosing effect's gate. Read the leaf's own gate
/// alone and the bot is handed an unconditional promise the campaign never made —
/// the same defect as dropping the gate entirely, one nesting level down.
#[test]
fn a_nested_drop_inherits_the_gate_of_every_effect_that_encloses_it() {
    let loaded = load();
    let out = build(&loaded, &campaign(&loaded, NESTED_UNDER_A_GATE));
    let plan = death_plan(&out);
    let terms = &drop_for(&plan, "stake/embers")["gates"][0]["terms"];
    assert_eq!(
        terms.as_array().map(Vec::len),
        Some(1),
        "the sequence's gate is the nested drop's gate: {terms}"
    );
    assert_eq!(terms[0]["objective"], "dw.s_embers");
    assert_eq!(terms[0]["min"], 3);
}
