//! DSL v0.10 (spec-0031 §"Status effect, granted and cleared"): the
//! `give-effect` / `clear-effect` pair, and the diagnostic that keeps a grant's
//! duration the thing that ends it.
//!
//! What this file pins down:
//! * the surface validates clean at `0.10.0` and is `DW0141` below it;
//! * an effect id outside the pinned 1.21.11 `mob_effect` registry is `DW0192` —
//!   the same code and the same registry a wave mob's `effects[]` answers to,
//!   because it is the same rule and one code names one rule;
//! * a duration of zero, a duration past the field width, and an amplifier past
//!   vanilla's unsigned byte are `DW0541`;
//! * **acceptance criterion 6**: a grant whose removal is a later effect in the
//!   same sequence is `DW0540`, and a grant whose duration expires before that
//!   removal is not — because then the duration is the removal, which is the
//!   whole point.
//!
//! There is deliberately no test for an *infinite* grant: `seconds` is a required
//! `u32` with no `infinite` spelling, so the hazard is inexpressible rather than
//! diagnosed. `the_grant_surface_has_no_infinite_form` asserts that from the
//! generated schema, so a future field cannot quietly reintroduce it.

mod common;

use delvewright_dsl::envelope::Stage;
use delvewright_dsl::{RawCampaign, check_campaign, stage_schema};
use serde_json::Value;

fn campaign_with(quests: &str) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    }
}

/// A quests document whose single objective-completion bundle is `effects`.
fn quests_with(version: &str, effects: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{version}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }}
        ],
        "on_objective_complete": {{ "obj/talk": {effects} }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn codes(quests: &str) -> Vec<String> {
    check_campaign(&campaign_with(quests))
        .into_iter()
        .map(|d| d.code)
        .collect()
}

/// The whole pair validates clean at `0.10.0`, including the `in` filter and the
/// gate every other verb carries.
#[test]
fn the_status_effect_pair_validates_clean() {
    let q = quests_with(
        "0.10.0",
        r#"[
      { "type": "give-effect", "effect": "minecraft:blindness", "seconds": 6,
        "amplifier": 1, "hide_particles": true,
        "in": { "anchor": "anchor/exit", "extent": [2, 3, 2] },
        "requires_flags": [], "forbids_flags": [] },
      { "type": "clear-effect", "effect": "minecraft:poison" }
    ]"#,
    );
    let d = check_campaign(&campaign_with(&q));
    assert!(d.is_empty(), "expected clean, got: {d:#?}");
}

/// Both verbs are reserved below `0.10.0` (`DW0141`), reported per effect.
#[test]
fn both_verbs_are_reserved_below_v10() {
    let q = quests_with(
        "0.9.0",
        r#"[
      { "type": "give-effect", "effect": "minecraft:speed", "seconds": 30 },
      { "type": "clear-effect", "effect": "minecraft:speed" }
    ]"#,
    );
    let d = check_campaign(&campaign_with(&q));
    for verb in ["give-effect", "clear-effect"] {
        assert!(
            d.iter()
                .any(|x| x.code == "DW0141" && x.message.contains(verb)),
            "`{verb}` must be reserved below 0.10.0: {d:#?}"
        );
    }
}

/// An id outside the pinned registry is `DW0192`, on either verb.
#[test]
fn an_unknown_effect_id_is_dw0192() {
    for verb in [
        r#"{ "type": "give-effect", "effect": "minecraft:courage", "seconds": 5 }"#,
        r#"{ "type": "clear-effect", "effect": "minecraft:courage" }"#,
    ] {
        let q = quests_with("0.10.0", &format!("[{verb}]"));
        assert!(
            codes(&q).contains(&"DW0192".to_string()),
            "unknown effect id must be DW0192 for {verb}"
        );
    }
}

/// An un-namespaced id is the SAME effect — vanilla accepts it, the pinned
/// registry normalizes it, and so does `DW0540`'s pairing. A grant spelled
/// `blindness` cleared by `minecraft:blindness` is one effect, not two, and this
/// is the test that says so: an exact string compare in the rule would report
/// clean here.
#[test]
fn a_bare_effect_id_is_the_same_effect() {
    let bare = quests_with(
        "0.10.0",
        r#"[{ "type": "give-effect", "effect": "blindness", "seconds": 5 }]"#,
    );
    let d = check_campaign(&campaign_with(&bare));
    assert!(d.is_empty(), "a bare id is valid: {d:#?}");

    let mixed = quests_with(
        "0.10.0",
        r#"[
      { "type": "give-effect", "effect": "blindness", "seconds": 60 },
      { "type": "clear-effect", "effect": "minecraft:blindness" }
    ]"#,
    );
    assert!(
        codes(&mixed).contains(&"DW0540".to_string()),
        "two spellings of one effect must pair"
    );
}

/// Zero seconds, a duration past the field width, and an over-wide amplifier are
/// each `DW0541`.
#[test]
fn out_of_range_duration_or_amplifier_is_dw0541() {
    for effect in [
        r#"{ "type": "give-effect", "effect": "minecraft:speed", "seconds": 0 }"#,
        r#"{ "type": "give-effect", "effect": "minecraft:speed", "seconds": 999999 }"#,
        r#"{ "type": "give-effect", "effect": "minecraft:speed", "seconds": 5,
             "amplifier": 300 }"#,
    ] {
        let q = quests_with("0.10.0", &format!("[{effect}]"));
        assert!(
            codes(&q).contains(&"DW0541".to_string()),
            "expected DW0541 for {effect}, got {:#?}",
            check_campaign(&campaign_with(&q))
        );
    }
}

/// **Acceptance criterion 6.** A `sequence` that grants blindness for the ride
/// and clears it at the end: the clear fires while the grant is still live, so
/// the clear — not the duration — is what ends it. `DW0540`.
#[test]
fn a_grant_removed_by_a_later_effect_in_the_same_sequence_is_dw0540() {
    let q = quests_with(
        "0.10.0",
        r#"[
      { "type": "sequence", "steps": [
        { "at_ticks": 0, "effects": [
          { "type": "give-effect", "effect": "minecraft:blindness", "seconds": 60 } ] },
        { "at_ticks": 4, "effects": [
          { "type": "clear-effect", "effect": "minecraft:blindness" } ] }
      ] }
    ]"#,
    );
    let d = check_campaign(&campaign_with(&q));
    let hit = d
        .iter()
        .find(|x| x.code == "DW0540")
        .unwrap_or_else(|| panic!("expected DW0540, got: {d:#?}"));
    // The message must carry the two numbers the author needs — how long the
    // grant runs and how long the sequence actually needs it for.
    assert!(hit.message.contains("60s"), "{}", hit.message);
    assert!(hit.message.contains("4 tick(s) later"), "{}", hit.message);
}

/// A bare `clear-effect` (vanilla's "clear everything") removes the grant just as
/// surely, so it is the same finding — the rule keys on what the clear DOES, not
/// on whether it happens to name the effect.
#[test]
fn a_clear_all_also_triggers_dw0540() {
    let q = quests_with(
        "0.10.0",
        r#"[
      { "type": "sequence", "steps": [
        { "at_ticks": 0, "effects": [
          { "type": "give-effect", "effect": "minecraft:blindness", "seconds": 60 } ] },
        { "at_ticks": 10, "effects": [ { "type": "clear-effect" } ] }
      ] }
    ]"#,
    );
    assert!(codes(&q).contains(&"DW0540".to_string()));
}

/// The pattern the spec prescribes: grant for exactly as long as the sequence
/// needs, and let it expire. No clear, no diagnostic.
#[test]
fn a_self_limiting_grant_is_clean() {
    let q = quests_with(
        "0.10.0",
        r#"[
      { "type": "sequence", "steps": [
        { "at_ticks": 0, "effects": [
          { "type": "give-effect", "effect": "minecraft:blindness", "seconds": 1 } ] },
        { "at_ticks": 4, "effects": [ { "type": "set-flag", "flag": "flag/rode" } ] }
      ] }
    ]"#,
    );
    let d = check_campaign(&campaign_with(&q));
    assert!(
        !d.iter().any(|x| x.code == "DW0540"),
        "a grant with no clear is the prescribed shape: {d:#?}"
    );
}

/// …and a clear that arrives AFTER the duration has already expired is not the
/// removal, so it is not this finding. The rule fires on live grants only.
#[test]
fn a_clear_after_the_duration_expires_is_not_dw0540() {
    let q = quests_with(
        "0.10.0",
        r#"[
      { "type": "sequence", "steps": [
        { "at_ticks": 0, "effects": [
          { "type": "give-effect", "effect": "minecraft:blindness", "seconds": 1 } ] },
        { "at_ticks": 40, "effects": [
          { "type": "clear-effect", "effect": "minecraft:blindness" } ] }
      ] }
    ]"#,
    );
    let d = check_campaign(&campaign_with(&q));
    assert!(
        !d.iter().any(|x| x.code == "DW0540"),
        "20 ticks of grant, cleared at tick 40: the duration ended it. {d:#?}"
    );
}

/// A grant and a clear side by side in one flat bundle run on the same tick, so
/// the grant is live when the clear fires — the degenerate case of the same rule.
#[test]
fn a_grant_and_clear_in_one_flat_bundle_is_dw0540() {
    let q = quests_with(
        "0.10.0",
        r#"[
      { "type": "give-effect", "effect": "minecraft:glowing", "seconds": 30 },
      { "type": "clear-effect", "effect": "minecraft:glowing" }
    ]"#,
    );
    assert!(codes(&q).contains(&"DW0540".to_string()));
}

/// A clear of a DIFFERENT effect is not this rule.
#[test]
fn a_clear_of_another_effect_is_clean() {
    let q = quests_with(
        "0.10.0",
        r#"[
      { "type": "give-effect", "effect": "minecraft:blindness", "seconds": 60 },
      { "type": "clear-effect", "effect": "minecraft:poison" }
    ]"#,
    );
    let d = check_campaign(&campaign_with(&q));
    assert!(!d.iter().any(|x| x.code == "DW0540"), "{d:#?}");
}

/// **Declaration order is not tick order.** A `sequence` may declare its late
/// step above its early one; the rule must pair the grant with the clear that
/// really fires first, or it reports clean on the live removal it exists to
/// find. Here the at-100 clear is inert (the 2s grant is long gone) and the at-4
/// clear is the real one.
#[test]
fn the_removal_is_the_earliest_by_tick_not_by_declaration_order() {
    let q = quests_with(
        "0.10.0",
        r#"[
      { "type": "sequence", "steps": [
        { "at_ticks": 0, "effects": [
          { "type": "give-effect", "effect": "minecraft:blindness", "seconds": 2 } ] },
        { "at_ticks": 100, "effects": [
          { "type": "clear-effect", "effect": "minecraft:blindness" } ] },
        { "at_ticks": 4, "effects": [
          { "type": "clear-effect", "effect": "minecraft:blindness" } ] }
      ] }
    ]"#,
    );
    let d = check_campaign(&campaign_with(&q));
    let hit = d
        .iter()
        .find(|x| x.code == "DW0540")
        .unwrap_or_else(|| panic!("the at-4 clear is the removal, whatever the order: {d:#?}"));
    assert!(hit.message.contains("4 tick(s) later"), "{}", hit.message);
}

/// …and the same key ordering catches a clear declared ABOVE the grant but
/// scheduled after it. A skip-to-the-next-index scan misses this one entirely.
#[test]
fn a_clear_declared_above_the_grant_but_scheduled_after_it_is_dw0540() {
    let q = quests_with(
        "0.10.0",
        r#"[
      { "type": "sequence", "steps": [
        { "at_ticks": 6, "effects": [
          { "type": "clear-effect", "effect": "minecraft:blindness" } ] },
        { "at_ticks": 0, "effects": [
          { "type": "give-effect", "effect": "minecraft:blindness", "seconds": 30 } ] }
      ] }
    ]"#,
    );
    assert!(codes(&q).contains(&"DW0540".to_string()));
}

/// The rule reaches a grant nested inside a `trigger`'s bundle too — it is
/// seeded from `for_each_effect_root`, the closed root set, not from the quest
/// stage's `on_objective_complete` alone.
#[test]
fn the_rule_reaches_every_effect_root() {
    let q = r#"{
  "dsl_version": "0.10.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [ { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" } ],
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "triggers": [
      {
        "id": "trigger/lamp",
        "at": "anchor/exit",
        "on": { "on": "use" },
        "effects": [
          { "type": "give-effect", "effect": "minecraft:night_vision", "seconds": 120 },
          { "type": "clear-effect", "effect": "minecraft:night_vision" }
        ]
      }
    ]
  }
}"#;
    assert!(codes(q).contains(&"DW0540".to_string()));
}

/// **There is no way to spell an unbounded grant.** `seconds` is a required
/// integer in the generated schema — no `infinite`, no string alternative, no
/// optional-with-a-default — read from the schema the types produce, so a future
/// field that reintroduced the hazard would fail here rather than in a playtest.
#[test]
fn the_grant_surface_has_no_infinite_form() {
    let schema: Value = stage_schema(Stage::Quests);
    let variants = schema
        .pointer("/definitions/QuestEffect/oneOf")
        .or_else(|| schema.pointer("/$defs/QuestEffect/oneOf"))
        .and_then(Value::as_array)
        .expect("QuestEffect is a tagged union in the schema");
    let give = variants
        .iter()
        .find(|v| {
            v.pointer("/properties/type/const")
                .and_then(Value::as_str)
                .or_else(|| v.pointer("/properties/type/enum/0").and_then(Value::as_str))
                == Some("give-effect")
        })
        .expect("the schema declares a `give-effect` variant");
    let required: Vec<&str> = give
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    assert!(
        required.contains(&"seconds"),
        "`seconds` must be REQUIRED — an optional duration is an infinite grant by \
         default: {give:#?}"
    );
    let seconds = give
        .pointer("/properties/seconds")
        .expect("a `seconds` property");
    // The SHAPE, not the prose: `type: "integer"` and nothing beside it. A string
    // alternative, an `enum`, a `oneOf` or an `anyOf` is exactly where an
    // `"infinite"` spelling would enter, so each is refused by name.
    assert_eq!(
        seconds.get("type").and_then(Value::as_str),
        Some("integer"),
        "`seconds` must be a plain integer: {seconds:#?}"
    );
    for alt in ["enum", "oneOf", "anyOf", "allOf"] {
        assert!(
            seconds.get(alt).is_none(),
            "`seconds` must have no `{alt}` alternative — that is where an infinite \
             form gets in: {seconds:#?}"
        );
    }
}
