//! spec-0016 §1 (owner directive 2026-08-03) — **what is in the flask.**
//!
//! The kit `flask` marker shipped with no way to declare the bottle's contents,
//! so every flask compiled to `minecraft:potion` with no
//! `minecraft:potion_contents` component: the Uncraftable Potion, which grants
//! nothing however it is named. `contents` models vanilla's component field for
//! field — a named potion, a custom-effect list, or both, plus the colour
//! override — and two diagnostics keep it honest: `DW0487` refuses the
//! placeholder, `DW0486` refuses contents 1.21.11 cannot pour.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};
use serde_json::{Value, json};

/// The hello-world classes doc at `dsl_version` `version`, with `flask` spliced
/// into every kit as the given item + contents (`contents: null` → no field).
fn classes_with(version: &str, item: Value, contents: Value) -> String {
    let mut v: Value = serde_json::from_str(&common::read_valid("classes.json")).unwrap();
    v["dsl_version"] = json!(version);
    for class in v["content"]["classes"].as_array_mut().unwrap() {
        let mut entry = json!({ "item": item, "count": 3, "flask": true });
        if !contents.is_null() {
            entry["contents"] = contents.clone();
        }
        class["kit"].as_array_mut().unwrap().push(entry);
    }
    serde_json::to_string(&v).unwrap()
}

/// A campaign whose classes stage is `classes` and whose other stages are the
/// stock hello-world ones. No bonfire anywhere, so `DW0476` never fires and the
/// only findings are the potion ones under test.
fn campaign(classes: &str) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: classes.to_string(),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: common::read_valid("quests.json"),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
    }
}

/// Every code raised for a classes doc.
fn codes_for(classes: &str) -> Vec<String> {
    check_campaign(&campaign(classes))
        .into_iter()
        .map(|d| d.code)
        .collect()
}

/// Assert `code` is among the findings — and, for the negative cases, that the
/// document is otherwise clean.
fn assert_code(classes: &str, code: &str) {
    let diags = check_campaign(&campaign(classes));
    assert!(
        diags.iter().any(|d| d.code == code),
        "expected {code} for this kit, got: {diags:#?}"
    );
}

// ---------------------------------------------------------------------------
// The surface
// ---------------------------------------------------------------------------

/// A **named vanilla potion** — the shortest way to say "a real Potion of
/// Healing II" — validates clean at 0.8.0.
#[test]
fn a_named_potion_validates_clean() {
    let c = classes_with(
        "0.8.0",
        json!("minecraft:potion"),
        json!({ "potion": "minecraft:strong_healing" }),
    );
    assert!(
        codes_for(&c).is_empty(),
        "a named-potion flask must validate clean: {:#?}",
        check_campaign(&campaign(&c))
    );
}

/// **Custom effects** (with a colour override) are the other half of the vanilla
/// component: an instantaneous heal takes no duration, a lasting effect does.
#[test]
fn custom_effects_validate_clean() {
    let c = classes_with(
        "0.8.0",
        json!("minecraft:splash_potion"),
        json!({
            "effects": [
                { "effect": "minecraft:instant_health", "amplifier": 1 },
                { "effect": "minecraft:regeneration", "duration": 200, "amplifier": 0 }
            ],
            "color": "#ff9c30"
        }),
    );
    assert!(
        codes_for(&c).is_empty(),
        "a custom-effect flask must validate clean: {:#?}",
        check_campaign(&campaign(&c))
    );
}

/// Version fence: `contents` is v0.8 surface, and declaring it earlier is
/// `DW0141` — the same asymmetry the whole version ledger uses, so no pre-0.8
/// campaign's datapack can move by a byte.
#[test]
fn contents_reserved_before_0_8() {
    let c = classes_with(
        "0.7.0",
        json!("minecraft:potion"),
        json!({ "potion": "minecraft:healing" }),
    );
    assert_code(&c, "DW0141");
}

// ---------------------------------------------------------------------------
// DW0487 — the placeholder flask
// ---------------------------------------------------------------------------

/// The directive itself: a potion with nothing in it is a build error at 0.8.0.
#[test]
fn a_contents_less_potion_is_dw0487() {
    let c = classes_with("0.8.0", json!("minecraft:potion"), Value::Null);
    assert_code(&c, "DW0487");
}

/// It is about the item, not the flask marker: a tipped arrow with no contents
/// is the same Uncraftable item, flask or not.
#[test]
fn every_potion_bearing_item_owes_contents() {
    for item in [
        "minecraft:splash_potion",
        "minecraft:lingering_potion",
        "minecraft:tipped_arrow",
    ] {
        let c = classes_with("0.8.0", json!(item), Value::Null);
        assert_code(&c, "DW0487");
    }
}

/// The **requirement** side fires only at 0.8.0. A 0.7 campaign carrying a bare
/// potion keeps validating exactly as it did — it has no way to declare contents,
/// so demanding them would be a version break rather than a check.
#[test]
fn a_pre_0_8_potion_kit_item_is_untouched() {
    let c = classes_with("0.7.0", json!("minecraft:potion"), Value::Null);
    let codes = codes_for(&c);
    assert!(
        !codes.iter().any(|c| c == "DW0487"),
        "the placeholder requirement must not reach a 0.7 campaign: {codes:?}"
    );
}

/// An ordinary item owes nothing: only the four items that actually carry the
/// component are asked for contents.
#[test]
fn a_non_potion_kit_item_owes_no_contents() {
    let c = classes_with("0.8.0", json!("minecraft:bread"), Value::Null);
    assert!(
        codes_for(&c).is_empty(),
        "bread is not a potion: {:#?}",
        check_campaign(&campaign(&c))
    );
}

// ---------------------------------------------------------------------------
// DW0486 — contents 1.21.11 cannot pour
// ---------------------------------------------------------------------------

/// Contents on an item with no `minecraft:potion_contents` component: the game
/// would discard the data, so the compiler refuses it rather than shipping a
/// declaration that does nothing.
#[test]
fn contents_on_a_non_potion_item_is_dw0486() {
    let c = classes_with(
        "0.8.0",
        json!("minecraft:bread"),
        json!({ "potion": "minecraft:healing" }),
    );
    assert_code(&c, "DW0486");
}

/// Contents that declare neither a potion nor an effect still pour nothing.
#[test]
fn empty_contents_is_dw0486() {
    let c = classes_with("0.8.0", json!("minecraft:potion"), json!({}));
    assert_code(&c, "DW0486");
}

/// The potion id is checked against the pinned 1.21.11 `potion` registry —
/// including the 1.20.5+ spelling trap, where strength and duration are part of
/// the id rather than separate fields.
#[test]
fn an_unknown_potion_id_is_dw0486() {
    for bad in [
        "minecraft:healing_ii",
        "minecraft:estus",
        "minecraft:strong",
    ] {
        let c = classes_with("0.8.0", json!("minecraft:potion"), json!({ "potion": bad }));
        assert_code(&c, "DW0486");
    }
}

/// A custom effect's id is checked against the pinned status-effect registry.
#[test]
fn an_unknown_effect_id_is_dw0486() {
    let c = classes_with(
        "0.8.0",
        json!("minecraft:potion"),
        json!({ "effects": [{ "effect": "minecraft:estus", "duration": 20 }] }),
    );
    assert_code(&c, "DW0486");
}

/// The amplifier is vanilla's unsigned byte; 256 is off the end of the field.
#[test]
fn an_out_of_range_amplifier_is_dw0486() {
    let c = classes_with(
        "0.8.0",
        json!("minecraft:potion"),
        json!({ "effects": [{ "effect": "minecraft:instant_health", "amplifier": 256 }] }),
    );
    assert_code(&c, "DW0486");
}

/// Durations are in ticks, and both ends are bounded: zero ticks is nothing at
/// all, and a value past the ceiling is a duration typed in the wrong unit.
#[test]
fn an_out_of_range_duration_is_dw0486() {
    for dur in [0u64, 1_000_001] {
        let c = classes_with(
            "0.8.0",
            json!("minecraft:potion"),
            json!({ "effects": [{ "effect": "minecraft:regeneration", "duration": dur }] }),
        );
        assert_code(&c, "DW0486");
    }
}

/// A lasting effect with no duration would default to zero ticks in vanilla —
/// i.e. to nothing — which is the placeholder failure one layer down.
#[test]
fn a_lasting_effect_without_a_duration_is_dw0486() {
    let c = classes_with(
        "0.8.0",
        json!("minecraft:potion"),
        json!({ "effects": [{ "effect": "minecraft:regeneration" }] }),
    );
    assert_code(&c, "DW0486");
}

/// …and the mirror image: an instantaneous effect lands once on drinking, so a
/// duration on it is a sentence the game never reads. Saying so is the whole
/// point — the author who wrote it believed they had authored a heal over time.
#[test]
fn a_duration_on_an_instant_effect_is_dw0486() {
    for eff in ["minecraft:instant_health", "minecraft:instant_damage"] {
        let c = classes_with(
            "0.8.0",
            json!("minecraft:potion"),
            json!({ "effects": [{ "effect": eff, "duration": 600 }] }),
        );
        assert_code(&c, "DW0486");
    }
}

/// The colour override is `#rrggbb` or nothing.
#[test]
fn a_malformed_color_is_dw0486() {
    for bad in ["ff9c30", "#ff9c3", "#gggggg", "orange"] {
        let c = classes_with(
            "0.8.0",
            json!("minecraft:potion"),
            json!({ "potion": "minecraft:healing", "color": bad }),
        );
        assert_code(&c, "DW0486");
    }
}

/// Registry fidelity (2026-08-03): the pinned 1.21.11 `mob_effect` registry has
/// forty entries and the vendored list had thirty-nine — the one 1.21.11 addition
/// was missing, so a campaign naming it was rejected by a check that was merely
/// out of date. Both the potion path and wave-mob `effects` read this list.
#[test]
fn the_effect_registry_covers_every_1_21_11_effect() {
    let c = classes_with(
        "0.8.0",
        json!("minecraft:potion"),
        json!({ "effects": [{ "effect": "minecraft:breath_of_the_nautilus", "duration": 200 }] }),
    );
    assert!(
        codes_for(&c).is_empty(),
        "a real 1.21.11 effect must not be rejected as unknown: {:#?}",
        check_campaign(&campaign(&c))
    );
}
