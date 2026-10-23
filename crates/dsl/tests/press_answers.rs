//! spec-0117: a click target always answers — the validation-tier half of the
//! pressable surface. `prop` on every interaction point (`DW1059`), `answers`
//! owed in every state a press can be in (`DW1051`), refused in a state it never
//! has (`DW1052`), the l10n keys and the schema export.

mod common;

use delvewright_dsl::{DSL_VERSION, RawCampaign, check_campaign, l10n, parse_campaign};

/// hello-world's single quest, an `interact` parameterised on its own fields,
/// and a trigger list.
fn quests(fields: &str, triggers: &str) -> String {
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
          {{ "type": "interact", "id": "obj/pry", "anchor": "anchor/exit", "after": ["obj/talk"]{fields} }}
        ],
        "on_complete": [ {{ "type": "set-flag", "flag": "flag/pried" }}, {{ "type": "campaign-complete" }} ]
      }}
    ],
    "triggers": [{triggers}]
  }}
}}"#
    )
}

fn campaign_with(quests: String) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests,
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    }
}

const BELL: &str = r#", "prop": { "block": "minecraft:bell[attachment=floor,facing=south]" }"#;
const ANSWERED: &str = r#", "answers": { "not_yet": "The rope is tied up.", "done": "Rung." }"#;
const GATED: &str = r#", "requires_item": "minecraft:iron_sword", "answers": { "not_yet": "Not yet.", "missing_item": "The bar will not shift with bare hands.", "done": "Pried." }"#;

fn codes(fields: &str, triggers: &str) -> Vec<(String, String)> {
    check_campaign(&campaign_with(quests(fields, triggers)))
        .into_iter()
        .map(|d| (d.code.to_string(), d.path))
        .collect()
}

fn has(cs: &[(String, String)], code: &str, path_end: &str) -> bool {
    cs.iter().any(|(c, p)| c == code && p.ends_with(path_end))
}

/// The answered bell validates clean; so does a gated interact with all three
/// lines.
#[test]
fn an_answered_press_validates_clean() {
    assert_eq!(
        codes(&format!("{BELL}{ANSWERED}"), ""),
        Vec::<(String, String)>::new()
    );
    assert_eq!(
        codes(&format!("{BELL}{GATED}"), ""),
        Vec::<(String, String)>::new()
    );
}

/// `DW1051`: an `interact` with no `answers` names `not_yet` and `done`; with
/// `requires_item`, `missing_item` too; a released one owes `not_yet` alone.
#[test]
fn dw1051_a_silent_interact_is_refused_per_state() {
    let cs = codes(BELL, "");
    assert!(has(&cs, "DW1051", "/answers/not_yet"), "{cs:?}");
    assert!(has(&cs, "DW1051", "/answers/done"), "{cs:?}");
    let cs = codes(
        &format!("{BELL}, \"requires_item\": \"minecraft:iron_sword\""),
        "",
    );
    assert!(has(&cs, "DW1051", "/answers/missing_item"), "{cs:?}");
    let door = r#", "prop": { "block": "minecraft:spruce_door[facing=south,half=lower]" }, "release": true"#;
    let cs = codes(door, "");
    assert!(has(&cs, "DW1051", "/answers/not_yet"), "{cs:?}");
    assert!(
        !has(&cs, "DW1051", "/answers/done"),
        "a released door owes no done: {cs:?}"
    );
}

/// `DW1051`: the silent block — `release` on a block the game reports no press
/// of.
#[test]
fn dw1051_release_on_an_unobservable_block_is_the_silent_block() {
    let campfire = r#", "prop": { "block": "minecraft:campfire" }, "release": true, "answers": { "not_yet": "Cold." }"#;
    let cs = codes(campfire, "");
    assert!(has(&cs, "DW1051", "/release"), "{cs:?}");
}

/// `DW1052`: `missing_item` without `requires_item`, `done` on a released door,
/// and `release` on a bell are each refused naming the field; each passes one
/// edit away.
#[test]
fn dw1052_inert_declarations_are_refused() {
    let cs = codes(
        &format!(r#"{BELL}, "answers": {{ "not_yet": "a", "missing_item": "b", "done": "c" }}"#),
        "",
    );
    assert!(has(&cs, "DW1052", "/answers/missing_item"), "{cs:?}");
    let door = r#", "prop": { "block": "minecraft:spruce_door[facing=south,half=lower]" }, "release": true, "answers": { "not_yet": "Barred.", "done": "Open." }"#;
    assert!(has(&codes(door, ""), "DW1052", "/answers/done"));
    let door_ok = r#", "prop": { "block": "minecraft:spruce_door[facing=south,half=lower]" }, "release": true, "answers": { "not_yet": "Barred." }"#;
    assert_eq!(codes(door_ok, ""), Vec::<(String, String)>::new());
    let cs = codes(&format!("{BELL}{ANSWERED}, \"release\": true"), "");
    assert!(has(&cs, "DW1052", "/release"), "{cs:?}");
}

/// `DW1052` and `DW1051` over triggers: `done` on a repeatable ungated
/// trigger is inert; a `once` gated trigger owes both lines; a `serves`
/// trigger owes both; an ungated, repeatable, unserved one owes nothing.
#[test]
fn trigger_answers_follow_the_trigger_states() {
    let open = r#"{ "id": "trigger/tiller", "at": "anchor/exit", "on": { "on": "use" },
        "prop": { "block": "minecraft:lever[face=floor,facing=north]" }, "once": false,
        "effects": [ { "type": "narrate", "text": "Creak." } ] }"#;
    assert_eq!(
        codes(&format!("{BELL}{ANSWERED}"), open),
        Vec::<(String, String)>::new()
    );
    let inert = open.replace(
        r#""once": false,"#,
        r#""once": false, "answers": { "done": "Already." },"#,
    );
    assert!(has(
        &codes(&format!("{BELL}{ANSWERED}"), &inert),
        "DW1052",
        "/answers/done"
    ));
    let gated = r#"{ "id": "trigger/sluice", "at": "anchor/exit", "on": { "on": "use" },
        "prop": { "block": "minecraft:lever[face=wall,facing=east]" }, "once": true,
        "requires_flags": ["flag/pried"],
        "effects": [ { "type": "narrate", "text": "Open." } ] }"#;
    let cs = codes(&format!("{BELL}{ANSWERED}"), gated);
    assert!(has(&cs, "DW1051", "triggers/0/answers/not_yet"), "{cs:?}");
    assert!(has(&cs, "DW1051", "triggers/0/answers/done"), "{cs:?}");
    let served = r#"{ "id": "trigger/button", "at": "anchor/exit", "on": { "on": "use" },
        "prop": { "block": "minecraft:oak_button[face=wall,facing=south]" }, "once": false,
        "serves": { "objective": "obj/pry" },
        "effects": [ { "type": "narrate", "text": "Click." } ] }"#;
    let cs = codes(&format!("{BELL}{ANSWERED}"), served);
    assert!(has(&cs, "DW1051", "triggers/0/answers/not_yet"), "{cs:?}");
    assert!(has(&cs, "DW1051", "triggers/0/answers/done"), "{cs:?}");
}

/// `DW1059`: an `interact` with no `prop`, a `use` whose prop is air and a
/// door named by its upper half are each refused naming the arm.
#[test]
fn dw1059_a_press_target_that_is_not_a_real_block() {
    let cs = codes(ANSWERED, "");
    assert!(has(&cs, "DW1059", "/prop"), "{cs:?}");
    let upper = r#", "prop": { "block": "minecraft:spruce_door[facing=south,half=upper]" }, "answers": { "not_yet": "a", "done": "b" }"#;
    assert!(has(&codes(upper, ""), "DW1059", "/prop/block"));
    let air = r#"{ "id": "trigger/air", "at": "anchor/exit", "on": { "on": "use" },
        "prop": { "block": "minecraft:air" }, "once": false,
        "effects": [ { "type": "narrate", "text": "Nothing." } ] }"#;
    let cs = codes(&format!("{BELL}{ANSWERED}"), air);
    assert!(has(&cs, "DW1059", "triggers/0/prop/block"), "{cs:?}");
}

/// The declared loosening (spec-0117 §3a): a gated lever trigger, an interact
/// on a door, a chest and an iron door, and a strike on a stone each pass.
#[test]
fn dw1059_the_click_box_admits_every_real_block() {
    for block in [
        "minecraft:spruce_door[facing=south,half=lower]",
        "minecraft:chest[facing=west]",
        "minecraft:iron_door[facing=south,half=lower]",
        "minecraft:campfire",
    ] {
        let fields = format!(r#", "prop": {{ "block": "{block}" }}{ANSWERED}"#);
        assert_eq!(
            codes(&fields, ""),
            Vec::<(String, String)>::new(),
            "{block}"
        );
    }
    let boulder = r#"{ "id": "trigger/boulder", "at": "anchor/exit", "on": { "on": "strike" },
        "prop": { "block": "minecraft:cobbled_deepslate" }, "once": true, "audience": "presser",
        "answers": { "done": "Already gone." },
        "effects": [ { "type": "set-flag", "flag": "flag/struck" } ] }"#;
    assert_eq!(
        codes(&format!("{BELL}{ANSWERED}"), boulder),
        Vec::<(String, String)>::new()
    );
}

/// `DW0427`: `answers` on an `approach` names nobody.
#[test]
fn dw0427_answers_on_an_approach_are_refused() {
    let t = r#"{ "id": "trigger/near", "at": "anchor/exit", "on": { "on": "approach", "range": 3 },
        "answers": { "not_yet": "a" },
        "effects": [ { "type": "narrate", "text": "Near." } ] }"#;
    assert!(has(
        &codes(&format!("{BELL}{ANSWERED}"), t),
        "DW0427",
        "/answers"
    ));
}

/// The lines are spoken to a player, so they are translated like every other
/// player-visible string, as `Refusal`s.
#[test]
fn answers_enter_the_l10n_inventory() {
    let c = parse_campaign(&campaign_with(quests(&format!("{BELL}{GATED}"), ""))).expect("parses");
    let inv = l10n::inventory(&c);
    for (state, line) in [
        ("not_yet", "Not yet."),
        ("missing_item", "The bar will not shift with bare hands."),
        ("done", "Pried."),
    ] {
        let key = format!("obj.open-the-door.pry.answers.{state}");
        assert_eq!(inv.get(&key).map(String::as_str), Some(line), "{key}");
        assert_eq!(l10n::key_kind(&key), Some(l10n::TextKind::Refusal), "{key}");
    }
}

/// The stage-5 schema exports the surface and none of the removed fields.
#[test]
fn stage5_schema_exports_the_pressable_surface() {
    let schema = delvewright_dsl::schema::stage_schema(delvewright_dsl::Stage::Quests);
    let json = serde_json::to_string(&schema).unwrap();
    for field in [
        "\"answers\"",
        "\"not_yet\"",
        "\"missing_item\"",
        "\"done\"",
        "\"release\"",
        "\"serves\"",
    ] {
        assert!(json.contains(field), "stage-5 schema must carry {field}");
    }
    for gone in [
        "\"missing_item_hint\"",
        "\"sealed_hint\"",
        "\"marker_item\"",
    ] {
        assert!(!json.contains(gone), "stage-5 schema must not carry {gone}");
    }
}

/// A removed field is refused as an unknown field, with no shim.
#[test]
fn a_removed_field_is_an_unknown_field() {
    let fields = format!(r#"{BELL}{ANSWERED}, "missing_item_hint": "old""#);
    assert!(parse_campaign(&campaign_with(quests(&fields, ""))).is_err());
}
