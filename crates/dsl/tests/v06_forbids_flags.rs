//! DSL v0.6: negative flag gating — `forbids_flags[]` alongside `requires_flags`
//! on objectives, environment triggers, quest/trigger effects, dialogue options
//! and traps (round-6 staging primitives). An element is suppressed while ANY
//! listed flag is set.
//!
//! Owner motivation (island QA round 6): a strike-the-giant retaliation trigger
//! must be armed once `flag/sealed` is set but stand down the moment
//! `flag/asleep` is set — the wake trigger takes over; without a negative gate
//! that needs re-arm plumbing content cannot express.
//!
//! Contract under test:
//! * validates clean under `0.6.0` everywhere `requires_flags` is accepted;
//! * an unknown flag in `forbids_flags` gets the same `DW0172` treatment as in
//!   `requires_flags`;
//! * reserved (`DW0141`) under a pre-0.6 campaign, at every site;
//! * a completing dialogue option gated only by `forbids_flags` counts as gated
//!   for the `DW0191` deadlock guard (conservative: no temporal flag reasoning).

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A v0.6 quests doc exercising `forbids_flags` on an objective, a quest
/// effect, a trigger (trigger-level and effect-level). `flag/armed` and
/// `flag/stood-down` are both produced by `set-flag` effects.
const QUESTS_FORBIDS: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2,
            "after": ["obj/talk"], "forbids_flags": ["flag/stood-down"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "set-flag", "flag": "flag/armed" },
            { "type": "open-gate", "anchor": "anchor/door", "forbids_flags": ["flag/stood-down"] }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "triggers": [
      {
        "id": "trigger/retaliate",
        "at": "anchor/keeper-stand",
        "on": { "on": "strike" },
        "requires_flags": ["flag/armed"],
        "forbids_flags": ["flag/stood-down"],
        "effects": [
          { "type": "set-flag", "flag": "flag/stood-down",
            "forbids_flags": ["flag/stood-down"] }
        ]
      }
    ]
  }
}"#;

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

/// `forbids_flags` on objectives, effects, triggers and trigger effects
/// validates clean under 0.6.0.
#[test]
fn forbids_flags_validates_clean() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_FORBIDS));
    assert!(
        diags.is_empty(),
        "forbids_flags across all quests-stage sites must validate clean: {diags:#?}"
    );
}

/// An unknown flag in any `forbids_flags` list is `DW0172` — the same treatment
/// as `requires_flags` (a never-produced flag can never suppress anything).
#[test]
fn unknown_forbids_flag_is_dw0172_at_every_site() {
    // One sweep per site so each path is individually proven.
    for (site, needle) in [
        ("objective", r#""forbids_flags": ["flag/stood-down"] }"#),
        (
            "effect",
            r#""forbids_flags": ["flag/stood-down"] }
          ]"#,
        ),
    ] {
        let broken =
            QUESTS_FORBIDS.replacen(needle, &needle.replace("flag/stood-down", "flag/typo"), 1);
        let diags = check_campaign(&campaign_with_quests(&broken));
        assert!(
            diags
                .iter()
                .any(|d| d.code == "DW0172" && d.path.contains("forbids_flags")),
            "unknown flag in {site} forbids_flags must be DW0172: {diags:#?}"
        );
    }
    // Trigger-level list.
    let broken = QUESTS_FORBIDS.replacen(
        r#""forbids_flags": ["flag/stood-down"],"#,
        r#""forbids_flags": ["flag/typo"],"#,
        1,
    );
    let diags = check_campaign(&campaign_with_quests(&broken));
    assert!(
        diags
            .iter()
            .any(|d| d.code == "DW0172" && d.path.contains("/triggers/0/forbids_flags")),
        "unknown flag in trigger forbids_flags must be DW0172: {diags:#?}"
    );
}

/// Every `forbids_flags` site is reserved (`DW0141`) under a pre-0.6 campaign.
#[test]
fn forbids_flags_reserved_before_0_6() {
    let pre = QUESTS_FORBIDS.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&campaign_with_quests(&pre));
    let reserved_paths: Vec<&str> = diags
        .iter()
        .filter(|d| d.code == "DW0141" && d.path.contains("forbids_flags"))
        .map(|d| d.path.as_str())
        .collect();
    for expected in [
        "/content/quests/0/objectives/1/forbids_flags",
        "/content/quests/0/on_objective_complete/obj/talk/1/forbids_flags",
        "/content/triggers/0/forbids_flags",
        "/content/triggers/0/effects/0/forbids_flags",
    ] {
        assert!(
            reserved_paths.contains(&expected),
            "expected DW0141 at `{expected}`; got: {reserved_paths:#?} ({diags:#?})"
        );
    }
}

/// A `talk-to` whose only completing option is `forbids_flags`-gated is a
/// `DW0191` deadlock risk: the option can be suppressed at any point, and the
/// static analysis does no temporal reasoning about which flags end up set.
#[test]
fn forbids_only_completing_option_is_dw0191() {
    // hello-world's dialogue: the completing option gains a forbids gate (the
    // flag itself is produced by the quests doc above, so no DW0172 noise).
    let dialogue = common::read_valid("dialogue.json")
        .replacen("\"0.2.0\"", "\"0.6.0\"", 1)
        .replacen(
            r#""effects": ["#,
            r#""forbids_flags": ["flag/armed"], "effects": ["#,
            1,
        );
    let mut raw = campaign_with_quests(QUESTS_FORBIDS);
    raw.dialogue = dialogue;
    let diags = check_campaign(&raw);
    assert!(
        diags.iter().any(|d| d.code == "DW0191"),
        "a forbids-only-gated completing option must be DW0191: {diags:#?}"
    );
}

/// The same dialogue-option `forbids_flags` under a pre-0.6 dialogue stage is
/// reserved (`DW0141`).
#[test]
fn dialogue_option_forbids_reserved_before_0_6() {
    let dialogue = common::read_valid("dialogue.json").replacen(
        r#""effects": ["#,
        r#""forbids_flags": ["flag/armed"], "effects": ["#,
        1,
    );
    let mut raw = campaign_with_quests(QUESTS_FORBIDS);
    raw.dialogue = dialogue;
    let diags = check_campaign(&raw);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "DW0141" && d.path.contains("forbids_flags")),
        "dialogue option forbids_flags must be reserved under 0.2.0 (DW0141): {diags:#?}"
    );
}
