//! The nested-anchor silent-drop family: every anchor-bearing effect resolves at
//! **any** nesting depth, or it is a loud diagnostic.
//!
//! The referential scans used to be shallow — they enumerated effect variants by
//! hand over the top level of each effect list — so a typo'd anchor one level down
//! (a `sequence` step, a `move-*` `on_arrive`, a `set-checkpoint` `on_respawn`)
//! validated clean and then emitted *nothing* at build time: a gate that never
//! opens, a block never placed. Both scans now go through the single nesting
//! authority (`QuestEffect::nested_effect_lists`) and the single anchor authority
//! (`QuestEffect::anchor_refs`):
//!
//! - `DW0142` — an effect anchor no prefab provides, nested or top-level, in a
//!   quest **or** an environment trigger (triggers were not scanned at all).
//! - `DW0195` — a `talk-to` on an NPC a prerequisite quest despawns, now seeing
//!   nested `despawn-npc`s, while staying honest about branches (a flag-gated or
//!   lifecycle-reaction despawn is not guaranteed to run).

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

fn campaign_with_quests(quests: &str) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
    }
}

/// A hello-world v0.6 `quests` doc whose `obj/talk` completion fires `effects` (a
/// raw JSON array body, no surrounding brackets) and which optionally declares
/// `triggers`.
fn quests_doc(effects: &str, triggers: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {effects} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]{triggers}
  }}
}}"#
    )
}

/// A `sequence` whose single step opens `anchor`.
fn sequence_open_gate(anchor: &str) -> String {
    format!(
        r#"{{ "type": "sequence", "steps": [
             {{ "at_ticks": 0, "effects": [
                 {{ "type": "open-gate", "anchor": "{anchor}" }} ] }} ] }}"#
    )
}

// --- DW0142: nested / trigger effect anchors -------------------------------

/// The control: a gate opened from inside a `sequence` step, on a real anchor,
/// validates clean. The island nests its gate fills exactly this way, so this must
/// stay green — the deep scan tightens the rule, it does not forbid nesting.
#[test]
fn nested_open_gate_on_a_real_anchor_validates_clean() {
    let doc = quests_doc(&sequence_open_gate("anchor/door"), "");
    let diags = check_campaign(&campaign_with_quests(&doc));
    assert!(
        diags.is_empty(),
        "a nested open-gate on a prefab-provided anchor must validate clean: {diags:#?}"
    );
}

/// A typo'd anchor on an `open-gate` **nested in a `sequence` step** is `DW0142`.
/// This is the sharpest case of the family: shallow scans walked straight past it,
/// and emission then produced no `fill` at all — a door that never opens.
#[test]
fn typod_anchor_nested_in_a_sequence_is_dw0142() {
    let doc = quests_doc(&sequence_open_gate("anchor/dorr"), "");
    let diags = check_campaign(&campaign_with_quests(&doc));
    let hit = diags
        .iter()
        .find(|d| d.code == "DW0142")
        .unwrap_or_else(|| panic!("a typo'd nested open-gate anchor must be DW0142: {diags:#?}"));
    assert!(
        hit.path.contains("steps/0/effects/0"),
        "the diagnostic must point at the nested effect, not the sequence: {hit:#?}"
    );
}

/// A typo'd anchor two levels down — a `set-block` inside a `move-npc`'s
/// `on_arrive`, inside a `sequence` step — is still `DW0142`. Proves the scan
/// recurses rather than peeking one level.
#[test]
fn typod_anchor_two_levels_down_is_dw0142() {
    let effects = r#"{ "type": "sequence", "steps": [
        { "at_ticks": 0, "effects": [
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit",
              "on_arrive": [
                { "type": "set-block", "anchor": "anchor/nowhere",
                  "block": "minecraft:air" } ] } ] } ] }"#;
    let diags = check_campaign(&campaign_with_quests(&quests_doc(effects, "")));
    assert!(
        diags.iter().any(|d| d.code == "DW0142"),
        "a typo'd anchor inside on_arrive inside a sequence must be DW0142: {diags:#?}"
    );
}

/// A typo'd anchor in an **environment trigger**'s effects is `DW0142`. Triggers
/// carried no anchor scan at all: they are global (no owning area), so they
/// resolve against the union of every known area's anchors.
#[test]
fn typod_trigger_effect_anchor_is_dw0142() {
    let triggers = r#",
    "triggers": [
      { "id": "trigger/gong", "at": "anchor/exit", "on": { "on": "use" }, "once": true,
        "effects": [ { "type": "set-block", "anchor": "anchor/nowhere",
                       "block": "minecraft:air" } ] }
    ]"#;
    let doc = quests_doc(
        r#"{ "type": "open-gate", "anchor": "anchor/door" }"#,
        triggers,
    );
    let diags = check_campaign(&campaign_with_quests(&doc));
    let hit = diags
        .iter()
        .find(|d| d.code == "DW0142")
        .unwrap_or_else(|| panic!("a typo'd trigger effect anchor must be DW0142: {diags:#?}"));
    assert!(
        hit.path.starts_with("/content/triggers/0/"),
        "the diagnostic must point into the trigger: {hit:#?}"
    );
}

/// A trigger effect on a real anchor stays clean — the union scope must not
/// reject a legitimate cross-area reference.
#[test]
fn trigger_effect_on_a_real_anchor_validates_clean() {
    let triggers = r#",
    "triggers": [
      { "id": "trigger/gong", "at": "anchor/exit", "on": { "on": "use" }, "once": true,
        "effects": [ { "type": "set-block", "anchor": "anchor/exit",
                       "block": "minecraft:air" } ] }
    ]"#;
    let doc = quests_doc(
        r#"{ "type": "open-gate", "anchor": "anchor/door" }"#,
        triggers,
    );
    let diags = check_campaign(&campaign_with_quests(&doc));
    assert!(
        diags.is_empty(),
        "a trigger effect on a prefab-provided anchor must validate clean: {diags:#?}"
    );
}

// --- DW0195: nested despawn-npc, branch-honest ------------------------------

/// A `quests` doc with two quests: `quest/open-the-door` fires `despawn` on its
/// talk objective, and the dependent `quest/second` then tries to `talk-to` the
/// same NPC.
fn two_quest_doc(despawn: &str) -> (String, String) {
    let quests = format!(
        r#"{{
  "dsl_version": "0.6.0",
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
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }}, {despawn} ]
        }},
        "on_complete": []
      }},
      {{
        "id": "quest/second",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/again", "npc": "npc/keeper" }}
        ],
        "on_objective_complete": {{}},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    );
    let plan = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quest-plan",
  "content": {
    "quests": [
      { "id": "quest/open-the-door", "goal": "Open the door", "area": "area/keep",
        "npcs": ["npc/keeper"], "depends_on": [], "mandatory": true, "act": 1 },
      { "id": "quest/second", "goal": "Speak again", "area": "area/keep",
        "npcs": ["npc/keeper"], "depends_on": ["quest/open-the-door"], "mandatory": true, "act": 1 }
    ],
    "finale": "quest/second"
  }
}"#;
    (quests, plan.to_string())
}

fn check_two_quest(despawn: &str) -> Vec<delvewright_dsl::Diagnostic> {
    let (quests, plan) = two_quest_doc(despawn);
    check_campaign(&RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: plan,
        quests,
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
    })
    .to_vec()
}

/// A `despawn-npc` **nested in a `sequence` step** removes the NPC just as surely
/// as a top-level one, so a later `talk-to` on it is still `DW0195`. The shallow
/// scan this replaces reported nothing and shipped a `talk-to` on a corpse.
#[test]
fn nested_despawn_npc_still_yields_dw0195() {
    let despawn = r#"{ "type": "sequence", "steps": [
        { "at_ticks": 0, "effects": [
            { "type": "despawn-npc", "npc": "npc/keeper" } ] } ] }"#;
    let diags = check_two_quest(despawn);
    assert!(
        diags.iter().any(|d| d.code == "DW0195"),
        "a despawn-npc nested in a sequence must still be DW0195: {diags:#?}"
    );
}

/// A **flag-gated** nested `despawn-npc` is a branch, not a certainty, so it must
/// NOT raise `DW0195`. This is the island's Perimedes: he walks out of the cave
/// and despawns only on the flee branch, while the `talk-to`s that follow live on
/// the sealed-in branch. Branch-conditional reachability is `DW0204`'s job.
#[test]
fn flag_gated_nested_despawn_is_not_dw0195() {
    let despawn = r#"{ "type": "sequence", "steps": [
        { "at_ticks": 0, "effects": [
            { "type": "despawn-npc", "npc": "npc/keeper",
              "requires_flags": ["flag/fled"] } ] } ] }"#;
    let diags = check_two_quest(despawn);
    assert!(
        !diags.iter().any(|d| d.code == "DW0195"),
        "a flag-gated despawn is one branch, not a guarantee — no DW0195: {diags:#?}"
    );
}

/// A `despawn-npc` inside a `set-checkpoint`'s `on_respawn` only runs if a player
/// dies, so it is likewise not a guarantee — no `DW0195`.
#[test]
fn despawn_in_a_respawn_hook_is_not_dw0195() {
    let despawn = r#"{ "type": "set-checkpoint", "anchor": "anchor/exit",
        "on_respawn": [ { "type": "despawn-npc", "npc": "npc/keeper" } ] }"#;
    let diags = check_two_quest(despawn);
    assert!(
        !diags.iter().any(|d| d.code == "DW0195"),
        "an on_respawn despawn only fires on death — no DW0195: {diags:#?}"
    );
}
