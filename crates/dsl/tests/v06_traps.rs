//! DSL v0.6 traps (spec-0011): the stage-5 `traps[]` surface validates under
//! `0.6.0` and is reserved (`DW0141`) earlier. A trap whose `at`/`disarm.via`
//! anchor no prefab provides (or a duplicate/malformed id) is `DW0340`; a
//! `dispense` payload item outside the pinned registry is `DW0341`.
//!
//! Built on the hello-world casting/plan/dialogue with a v0.6 `quests` document
//! (additive `traps` field), so the rest is untouched. `check_campaign` injects
//! the vendored anchor registry, which knows hello-room's anchors (`spawn`,
//! `anchor/keeper-stand`, `anchor/door`, `anchor/exit`) — the DSL layer checks
//! anchor *names* only; the dispenser socket + completability proof are the
//! compiler's job (`DW0342`), covered in the compiler crate.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A v0.6 `quests` document carrying `traps`. `{VER}` / `{TRAPS}` are filled per
/// test. The quest body is the unchanged hello-world expansion.
const QUESTS_TMPL: &str = r#"{
  "dsl_version": "{VER}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/talk"] }
        ],
        "on_objective_complete": { "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ] },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "traps": [ {TRAPS} ]
  }
}"#;

fn campaign_with_traps(version: &str, traps: &str) -> RawCampaign {
    let quests = QUESTS_TMPL
        .replacen("{VER}", version, 1)
        .replacen("{TRAPS}", traps, 1);
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests,
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
    }
}

/// A well-formed dart trap bound to a provided anchor with an in-registry payload.
const VALID_TRAP: &str = r#"{
  "id": "trap/dart-hall",
  "at": "anchor/exit",
  "trigger": "trapped-chest",
  "effect": { "dispense": { "item": "minecraft:torch", "count": 8 } },
  "lethality": "harmful"
}"#;

/// A well-formed trap validates clean under `dsl_version 0.6.0`.
#[test]
fn v06_trap_validates_clean() {
    let diags = check_campaign(&campaign_with_traps("0.6.0", VALID_TRAP));
    assert!(
        diags.is_empty(),
        "a valid v0.6 trap must validate clean, got: {diags:#?}"
    );
}

/// The `traps` section is reserved under a pre-0.6 quests version -> `DW0141`.
#[test]
fn v06_traps_reserved_before_0_6() {
    let diags = check_campaign(&campaign_with_traps("0.5.0", VALID_TRAP));
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "traps must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}

/// A trap `at` an anchor no prefab provides is `DW0340`.
#[test]
fn v06_trap_bad_anchor_is_dw0340() {
    let trap = VALID_TRAP.replacen("anchor/exit", "anchor/nowhere", 1);
    let diags = check_campaign(&campaign_with_traps("0.6.0", &trap));
    assert!(
        diags.iter().any(|d| d.code == "DW0340"),
        "an `at` anchor no prefab provides must be DW0340: {diags:#?}"
    );
}

/// A `disarm.via` that collides with the trap's own `at` anchor is `DW0340`.
#[test]
fn v06_trap_disarm_collides_with_trigger_is_dw0340() {
    let trap = r#"{
      "id": "trap/dart-hall",
      "at": "anchor/exit",
      "trigger": "pressure-plate",
      "effect": { "dispense": { "item": "minecraft:torch", "count": 8 } },
      "lethality": "lethal",
      "disarm": { "via": "anchor/exit", "sets_flag": "flag/darts-off" }
    }"#;
    let diags = check_campaign(&campaign_with_traps("0.6.0", trap));
    assert!(
        diags.iter().any(|d| d.code == "DW0340"),
        "a disarm.via equal to the trap's own anchor must be DW0340: {diags:#?}"
    );
}

/// A dispense payload item outside the pinned 1.21.11 registry is `DW0341`.
#[test]
fn v06_trap_unknown_payload_is_dw0341() {
    let trap = VALID_TRAP.replacen("minecraft:torch", "minecraft:definitely-not-an-item", 1);
    let diags = check_campaign(&campaign_with_traps("0.6.0", &trap));
    assert!(
        diags.iter().any(|d| d.code == "DW0341"),
        "an unknown dispense payload item must be DW0341: {diags:#?}"
    );
}

/// A non-`dispense` effect key (e.g. TNT) is an unknown enum variant -> `DW0100`,
/// keeping block-destroying/unmodeled effects out of the schema (spec-0011).
#[test]
fn v06_trap_tnt_effect_is_rejected() {
    let trap = r#"{
      "id": "trap/boom",
      "at": "anchor/exit",
      "trigger": "pressure-plate",
      "effect": { "tnt": { "power": 4 } },
      "lethality": "lethal"
    }"#;
    let diags = check_campaign(&campaign_with_traps("0.6.0", trap));
    assert!(
        diags.iter().any(|d| d.code == "DW0100"),
        "a non-dispense trap effect (tnt) must be rejected as an unknown variant (DW0100): {diags:#?}"
    );
}

/// v0.6 negative gate: an unknown flag in a trap's `forbids_flags` is `DW0172`
/// (same treatment as `requires_flags` — round-6 staging primitives).
#[test]
fn v06_trap_unknown_forbids_flag_is_dw0172() {
    let trap = VALID_TRAP.replacen(
        "\"lethality\": \"harmful\"",
        "\"lethality\": \"harmful\",\n  \"forbids_flags\": [\"flag/never-produced\"]",
        1,
    );
    let diags = check_campaign(&campaign_with_traps("0.6.0", &trap));
    assert!(
        diags
            .iter()
            .any(|d| d.code == "DW0172" && d.path.contains("/traps/0/forbids_flags")),
        "unknown flag in trap forbids_flags must be DW0172: {diags:#?}"
    );
}
