//! DSL v0.6 (spec-0013): the stage-1 `horizon` and `boundary` world fields
//! validate under `0.6.0` and are reserved (`DW0141`) earlier. Under 0.6.0,
//! `horizon: "ocean"` without a `boundary` is `DW0320` and a `boundary.margin`
//! outside `0..=64` is `DW0321`.
//!
//! Built on the hello-world casting/quests/dialogue (unchanged, 0.2.0) with a
//! v0.6 stage-1 `world` document — additive fields, so the rest is untouched.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign, l10n_inventory, localize, parse_campaign};

/// A v0.6 stage-1 world document: ocean horizon + a boundary (the happy path).
const WORLD_V06: &str = r#"{
  "dsl_version": "0.19.0",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "horizon": "ocean",
    "boundary": { "margin": 24, "message": "The tide turns you back." },
    "areas": [
      { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" }
    ]
  }
}"#;

/// Ocean horizon with NO boundary — the `DW0320` authoring error.
const WORLD_V06_OCEAN_NO_BOUNDARY: &str = r#"{
  "dsl_version": "0.19.0",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "horizon": "ocean",
    "areas": [
      { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" }
    ]
  }
}"#;

/// Explicit void horizon, no boundary — valid (void needs no return rule).
const WORLD_V06_VOID: &str = r#"{
  "dsl_version": "0.19.0",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "horizon": "void",
    "areas": [
      { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" }
    ]
  }
}"#;

fn campaign_with_world(world: &str) -> RawCampaign {
    RawCampaign {
        world: world.to_string(),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: common::read_valid("quests.json"),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    }
}

/// v0.6 horizon + boundary validate clean under `dsl_version 0.6.0`.
#[test]
fn v06_world_surface_validates_clean() {
    let diags = check_campaign(&campaign_with_world(WORLD_V06));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for the v0.6 world surface, got: {diags:#?}"
    );
}

/// `horizon: "ocean"` without a `boundary` is `DW0320`.
#[test]
fn v06_ocean_without_boundary_is_dw0320() {
    let diags = check_campaign(&campaign_with_world(WORLD_V06_OCEAN_NO_BOUNDARY));
    assert!(
        diags.iter().any(|d| d.code == "DW0320"),
        "ocean without boundary must be DW0320: {diags:#?}"
    );
}

/// `boundary.margin` above 64 is `DW0321`.
#[test]
fn v06_margin_out_of_range_is_dw0321() {
    let bad = WORLD_V06.replacen("\"margin\": 24", "\"margin\": 65", 1);
    let diags = check_campaign(&campaign_with_world(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0321"),
        "margin 65 must be DW0321 (range 0..=64): {diags:#?}"
    );
}

/// `margin: 0` is in range (0..=64 inclusive) — no `DW0321`.
#[test]
fn v06_margin_zero_is_in_range() {
    let ok = WORLD_V06.replacen("\"margin\": 24", "\"margin\": 0", 1);
    let diags = check_campaign(&campaign_with_world(&ok));
    assert!(
        !diags.iter().any(|d| d.code == "DW0321"),
        "margin 0 is in range and must not be DW0321: {diags:#?}"
    );
}

/// An explicit `void` horizon with no boundary validates clean.
#[test]
fn v06_void_horizon_needs_no_boundary() {
    let diags = check_campaign(&campaign_with_world(WORLD_V06_VOID));
    assert!(
        diags.is_empty(),
        "explicit void horizon needs no boundary: {diags:#?}"
    );
}

// ---------------------------------------------------------------------------
// v0.6 per-effect `requires_flags` gate + blockstate suffix
// ---------------------------------------------------------------------------

/// Build a full campaign with a custom stage-5 `quests` document (0.6.0), reusing
/// the valid hello-world documents for every other stage.
fn campaign_with_quests(quests: &str) -> RawCampaign {
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

/// A 0.6.0 quests document: an `open-gate` effect gated on a flag the same
/// objective sets first (the happy path for per-effect `requires_flags`).
const QUESTS_V06_GATED_EFFECT: &str = r#"{
  "dsl_version": "0.19.0",
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
        "on_objective_complete": {
          "obj/talk": [
            { "type": "set-flag", "flag": "flag/opened" },
            { "type": "open-gate", "anchor": "anchor/door", "requires_flags": ["flag/opened"] }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

/// A per-effect `requires_flags` that references a flag no `set-flag` produces.
const QUESTS_V06_GATED_EFFECT_UNKNOWN_FLAG: &str = r#"{
  "dsl_version": "0.19.0",
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
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door", "requires_flags": ["flag/never-set"] }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

/// A 0.6.0 quests document placing a block that carries a vanilla blockstate.
const QUESTS_V06_BLOCKSTATE: &str = r#"{
  "dsl_version": "0.19.0",
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
        "on_objective_complete": {
          "obj/talk": [
            { "type": "set-block", "anchor": "anchor/door", "block": "minecraft:water[level=0]" }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

/// A per-effect flag gate that resolves validates clean under 0.6.0.
#[test]
fn v06_effect_requires_flags_validates_clean() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V06_GATED_EFFECT));
    assert!(
        diags.is_empty(),
        "a resolved per-effect requires_flags must validate clean at 0.6.0: {diags:#?}"
    );
}

/// A per-effect `requires_flags` referencing an unproduced flag is `DW0172`.
#[test]
fn v06_effect_requires_flags_unknown_is_dw0172() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V06_GATED_EFFECT_UNKNOWN_FLAG));
    assert!(
        diags.iter().any(|d| d.code == "DW0172"),
        "an unproduced effect requires_flags must be DW0172: {diags:#?}"
    );
}

/// A flag produced **only** by a `set-flag` nested inside a `sequence` step is a
/// real producer: a later `requires_flags` referencing it must resolve, not
/// spuriously trip `DW0172`. Regression for the shallow producer scan that skipped
/// nested `set-flag`s.
const QUESTS_V06_SEQUENCE_SETS_FLAG: &str = r#"{
  "dsl_version": "0.19.0",
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
        "on_objective_complete": {
          "obj/talk": [
            { "type": "sequence", "steps": [
              { "at_ticks": 0, "effects": [ { "type": "set-flag", "flag": "flag/opened" } ] }
            ] },
            { "type": "open-gate", "anchor": "anchor/door", "requires_flags": ["flag/opened"] }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

/// A quests doc with a `narrate` nested inside a `sequence` step (the Q4/Q7
/// cinematic shape): its player-visible text must be inventoried under a stable,
/// position-derived nested key so a translated build ships it localized instead of
/// English-only.
const QUESTS_V06_SEQUENCE_NARRATE: &str = r#"{
  "dsl_version": "0.19.0",
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
        "on_objective_complete": {
          "obj/talk": [
            { "type": "sequence", "steps": [
              { "at_ticks": 0, "effects": [ { "type": "narrate", "text": "Top-of-step line." } ] },
              { "at_ticks": 40, "effects": [ { "type": "narrate", "text": "The seal cracks open." } ] }
            ] }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

/// The nested-narrate key for a `sequence` narrate in quest `open-the-door`,
/// objective `talk`, top-level effect 0, step 1, nested effect 0.
const NESTED_NARRATE_KEY: &str = "fx.open-the-door.oc.talk.0.seq.1.0.narrate";

/// A `narrate` nested inside a `sequence` step enters the l10n inventory under a
/// stable, position-derived key, and `localize` swaps it — so a translated build
/// no longer ships nested cinematic narration English-only. Regression for the
/// shallow inventory that only walked top-level effects.
#[test]
fn v06_sequence_narrate_is_inventoried_and_localized() {
    let raw = campaign_with_quests(QUESTS_V06_SEQUENCE_NARRATE);
    assert!(
        check_campaign(&raw).is_empty(),
        "the sequence-narrate campaign validates clean: {:#?}",
        check_campaign(&raw)
    );
    let mut campaign = parse_campaign(&raw).expect("parses");

    // The nested narrate is inventoried with its canonical English text.
    let inv = l10n_inventory(&campaign);
    assert_eq!(
        inv.get(NESTED_NARRATE_KEY).map(String::as_str),
        Some("The seal cracks open."),
        "sequence narrate must be inventoried under a stable nested key; inventory: {inv:#?}"
    );
    // Determinism: the key derivation is stable across builds (byte-identity gate).
    assert_eq!(inv, l10n_inventory(&campaign), "inventory is deterministic");

    // Localize swaps the nested narrate in place (the emission path reads this).
    let mut tr = std::collections::BTreeMap::new();
    tr.insert(NESTED_NARRATE_KEY.to_string(), "封印裂开了。".to_string());
    localize(&mut campaign, &tr);
    let oid = delvewright_dsl::ObjectiveId("obj/talk".to_string());
    let narrate_texts: Vec<&str> = campaign.quests.content.quests[0].on_objective_complete[&oid]
        .iter()
        .flat_map(|e| e.nested_effect_lists())
        .flatten()
        .filter_map(|e| match e {
            delvewright_dsl::QuestEffect::Narrate { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        narrate_texts.contains(&"封印裂开了。"),
        "localize must swap the nested sequence narrate, got {narrate_texts:?}"
    );
}

#[test]
fn v06_set_flag_nested_in_sequence_is_a_producer_no_dw0172() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V06_SEQUENCE_SETS_FLAG));
    assert!(
        !diags.iter().any(|d| d.code == "DW0172"),
        "a set-flag nested in a sequence produces its flag — requires_flags must \
         resolve, no spurious DW0172: {diags:#?}"
    );
    assert!(
        diags.is_empty(),
        "the nested-set-flag campaign must validate clean: {diags:#?}"
    );
}

/// A block field carrying a well-formed vanilla blockstate validates clean.
#[test]
fn v06_blockstate_suffix_validates_clean() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V06_BLOCKSTATE));
    assert!(
        diags.is_empty(),
        "a well-formed blockstate suffix must validate clean: {diags:#?}"
    );
}

/// A malformed blockstate suffix reuses the invalid-block diagnostic `DW0193`.
#[test]
fn v06_malformed_blockstate_is_dw0193() {
    let bad =
        QUESTS_V06_BLOCKSTATE.replacen("minecraft:water[level=0]", "minecraft:water[level=0", 1);
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0193"),
        "an unbalanced blockstate suffix must be DW0193: {diags:#?}"
    );
}

/// An unknown base id with a well-formed state suffix is still `DW0193`.
#[test]
fn v06_blockstate_base_id_still_registry_checked() {
    let bad = QUESTS_V06_BLOCKSTATE.replacen(
        "minecraft:water[level=0]",
        "minecraft:not_a_block[level=0]",
        1,
    );
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0193"),
        "an unknown base id must still be DW0193 even with a valid state: {diags:#?}"
    );
}
