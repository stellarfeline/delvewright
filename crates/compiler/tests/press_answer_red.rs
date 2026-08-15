//! The two reds, in a form that COMPILES on `origin/main` as well as on this
//! branch — no new API is touched, only the DSL parser and the validator.
//!
//! Run it on either tree with:
//!   cp <this file> crates/compiler/tests/press_answer_red.rs
//!   cargo test -p delvec --test press_answer_red

mod common;

use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, EnvTrigger, parse_campaign, validate_campaign_with};

/// The `souls-shortcut` fixture: a doorway slab sealed from world-load, opened
/// from the far side, carrying the author's own **left**-click line. Its
/// right-click — the press a shortcut loop invites — is the silence.
fn fixture() -> Campaign {
    let dir = common::compiler_fixtures_dir().join("souls-shortcut");
    let loaded = load_campaign_dir(&dir).unwrap();
    parse_campaign(&loaded.raw).expect("souls-shortcut parses")
}

fn diagnostics(c: &Campaign) -> Vec<delvewright_dsl::Diagnostic> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    validate_campaign_with(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
}

/// The campaign's own wrong-side answer, on the general click verb.
const ANSWER: &str = r#"{ "id": "trigger/from-the-wrong-side", "at": "anchor/door",
     "on": { "on": "use" }, "once": false, "audience": "presser",
     "effects": [ { "type": "narrate", "style": "actionbar",
                    "text": "The door cannot be opened from this side." } ] }"#;

/// **RED 1 — a barred door with nothing to say ships.**
///
/// The campaign seals a doorway the party is invited to walk up to and push on,
/// and nothing anywhere answers a right-click on it. That must not compile: the
/// press produces silence, and the compiler will not word the door on the
/// author's behalf.
///
/// On `origin/main` there is no such obligation, so the campaign is accepted and
/// ships the silence. This is the direction that drifts — a door is added, nobody
/// writes its line, and every board stays green.
#[test]
fn a_barred_door_with_nothing_to_say_is_refused() {
    let mut c = fixture();
    c.quests.dsl_version = "0.11.0".to_string();
    let diags = diagnostics(&c);
    assert!(
        diags.iter().any(|d| d.code == "DW0429"),
        "a shortcut door that answers no press must be REFUSED (DW0429). \
         The campaign was accepted with: {diags:#?}"
    );
}

/// **RED 1b — the same silence, on the other object of the same class.**
///
/// A `close-gate` that seals a wall and authors no `sealed_hint` is the identical
/// defect: the party presses the stone and the compiler decides what it says. On
/// `origin/main` it is not merely accepted, it is *defaulted* — the engine picks
/// the tone and never discloses that it did.
///
/// The two must be refused by ONE rule. Two objects of one class with two
/// defaulting policies is the "capability keyed to the verb" defect this whole
/// surface is the worked example of.
#[test]
fn an_unauthored_seal_is_refused_by_the_same_rule() {
    let hw = |n: &str| std::fs::read_to_string(common::hello_world_dir().join(n)).unwrap();
    let quests = r#"{
  "dsl_version": "0.11.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "id": "obj/talk", "type": "talk-to", "npc": "npc/keeper" },
          { "id": "obj/exit", "type": "reach-anchor", "anchor": "anchor/exit",
            "radius": 2, "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]
        },
        "on_complete": [
          { "type": "close-gate", "anchor": "anchor/door",
            "happening": { "verb": "seals", "text": "The bars come down." } },
          { "type": "campaign-complete",
            "happening": { "verb": "survives", "text": "The party is out." } }
        ]
      }
    ]
  }
}"#;
    let raw = delvewright_dsl::RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: hw("dialogue.json"),
        world_edits: None,
    };
    let c = parse_campaign(&raw).expect("campaign parses");
    let diags = diagnostics(&c);
    assert!(
        diags.iter().any(|d| d.code == "DW0429"),
        "a sealed wall nobody worded must be REFUSED by the same rule the door is. \
         The campaign was accepted with: {diags:#?}"
    );
}

/// **RED 2 — and the campaign could not have written that line anyway.**
///
/// The general verb is `EnvTrigger{on: use}` + `narrate`. To say what a
/// wrong-side answer says it needs the reply CHANNEL (`actionbar`) and the
/// ADDRESSEE (`presser`). On `origin/main` neither exists, so the document does
/// not even parse — which is what made the refusal above impossible to demand.
#[test]
fn the_campaign_can_write_a_wrong_side_answer() {
    let t = serde_json::from_str::<EnvTrigger>(ANSWER).unwrap_or_else(|e| {
        panic!("a campaign CANNOT express a wrong-side press answer on the general verb: {e}")
    });
    let mut c = fixture();
    c.quests.dsl_version = "0.11.0".to_string();
    c.quests.content.triggers.push(t);
    let diags = diagnostics(&c);
    assert!(
        diags.is_empty(),
        "one authored line must clear the refusal: {diags:#?}"
    );
}
