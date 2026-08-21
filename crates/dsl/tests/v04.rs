//! DSL v0.4 (spec-0008 + addendum, spec-0009) negative tests for the v0.4-only
//! semantic checks (`v04_checks` in `validate.rs`): mannequin skins, wave-mob
//! `effects`, `set-block` block ids, environment triggers, and the
//! despawned-npc `talk-to` guard. Each test bumps only the stage(s) that carry
//! the v0.4 construct under test to `dsl_version 0.4.0` (avoiding an incidental
//! `DW0141` reserved-feature diagnostic on an unrelated stage) and asserts the
//! expected code fires.
//!
//! Built on the hello-world fixture campaign (`crates/dsl/fixtures/valid/
//! hello-world`), matching the `v03.rs`/`v05.rs` pattern (literal JSON +
//! `check_campaign`, `.any(|d| d.code == "DWxxxx")`).

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

fn campaign_with(npcs: &str, quests: &str, dialogue: &str) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: npcs.to_string(),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: dialogue.to_string(),
        world_edits: None,
    }
    geometry_brief: None,
    layout_graph: None,
}

fn valid_npcs_v04() -> String {
    common::read_valid("npcs.json").replacen("\"0.2.0\"", "\"0.4.0\"", 1)
}

fn valid_dialogue_v04() -> String {
    common::read_valid("dialogue.json").replacen("\"0.2.0\"", "\"0.4.0\"", 1)
}

/// The hello-world quests document, at v0.4.0, with no v0.4 constructs — used as
/// the base that individual tests inject one bad construct into.
const QUESTS_BASE: &str = r#"{
  "dsl_version": "0.4.0",
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
          "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

#[test]
fn v04_base_campaign_validates_clean() {
    let diags = check_campaign(&campaign_with(
        &valid_npcs_v04(),
        QUESTS_BASE,
        &valid_dialogue_v04(),
    ));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for the v0.4 base campaign, got: {diags:#?}"
    );
}

// ---------------------------------------------------------------------------
// DW0190 — mannequin skin malformed / duplicated
// ---------------------------------------------------------------------------

#[test]
fn malformed_skin_texture_id_is_dw0190() {
    let npcs = valid_npcs_v04().replacen(
        "\"base_entity\": \"minecraft:villager\",",
        "\"base_entity\": \"minecraft:villager\", \"skin\": { \"texture_id\": \"Not Kebab!\", \
         \"model\": \"wide\" },",
        1,
    );
    let diags = check_campaign(&campaign_with(&npcs, QUESTS_BASE, &valid_dialogue_v04()));
    assert!(
        diags.iter().any(|d| d.code == "DW0190"),
        "malformed texture_id must be DW0190: {diags:#?}"
    );
}

// ---------------------------------------------------------------------------
// DW0191 — talk-to with only flag-gated completing options (deadlock risk)
// ---------------------------------------------------------------------------

#[test]
fn all_completing_options_flag_gated_is_dw0191() {
    // Gate the only completing option on a flag nothing ever sets: every
    // completing option for `obj/talk` becomes requires_flags-gated.
    let dialogue = common::patch_doc(&valid_dialogue_v04(), |d| {
        let opts = d["content"]["dialogues"][0]["nodes"][0]["options"]
            .as_array_mut()
            .expect("the greeting node has options");
        let completing = opts
            .iter_mut()
            .find(|o| o["effects"][0]["type"] == "complete-objective")
            .expect("the greeting node still holds the only completing option");
        completing["requires_flags"] = serde_json::json!(["flag/never-set"]);
    });
    let diags = check_campaign(&campaign_with(&valid_npcs_v04(), QUESTS_BASE, &dialogue));
    assert!(
        diags.iter().any(|d| d.code == "DW0191"),
        "an all-flag-gated talk-to must be DW0191: {diags:#?}"
    );
}

// ---------------------------------------------------------------------------
// DW0192 — wave-mob `effects[].effect` not a known status-effect id
// ---------------------------------------------------------------------------

const QUESTS_BAD_WAVE_EFFECT: &str = r#"{
  "dsl_version": "0.4.0",
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
          "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "waves": [
      {
        "id": "wave/ambush",
        "anchor": "anchor/keeper-stand",
        "mobs": [
          { "entity": "minecraft:zombie", "count": 1, "effects": [ { "effect": "minecraft:not_a_real_effect", "amplifier": 0 } ] }
        ]
      }
    ]
  }
}"#;

#[test]
fn unknown_wave_mob_effect_is_dw0192() {
    let diags = check_campaign(&campaign_with(
        &valid_npcs_v04(),
        QUESTS_BAD_WAVE_EFFECT,
        &valid_dialogue_v04(),
    ));
    assert!(
        diags.iter().any(|d| d.code == "DW0192"),
        "unknown wave-mob effect id must be DW0192: {diags:#?}"
    );
}

// ---------------------------------------------------------------------------
// DW0193 — `set-block` / `interact.prop` block id not a known 1.21.11 block
// ---------------------------------------------------------------------------

#[test]
fn unknown_set_block_id_is_dw0193() {
    let quests = QUESTS_BASE.replacen(
        "{ \"type\": \"open-gate\", \"anchor\": \"anchor/door\" }",
        "{ \"type\": \"open-gate\", \"anchor\": \"anchor/door\" }, \
         { \"type\": \"set-block\", \"anchor\": \"anchor/exit\", \"block\": \"minecraft:not_a_real_block\" }",
        1,
    );
    let diags = check_campaign(&campaign_with(
        &valid_npcs_v04(),
        &quests,
        &valid_dialogue_v04(),
    ));
    assert!(
        diags.iter().any(|d| d.code == "DW0193"),
        "unknown set-block block id must be DW0193: {diags:#?}"
    );
}

// ---------------------------------------------------------------------------
// DW0194 — environment-trigger id malformed/duplicated, or `approach` range 0
// ---------------------------------------------------------------------------

const QUESTS_BAD_TRIGGER: &str = r#"{
  "dsl_version": "0.4.0",
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
          "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "triggers": [
      {
        "id": "not-a-valid-trigger-id",
        "at": "anchor/exit",
        "on": { "on": "strike" },
        "effects": []
      }
    ]
  }
}"#;

#[test]
fn malformed_trigger_id_is_dw0194() {
    let diags = check_campaign(&campaign_with(
        &valid_npcs_v04(),
        QUESTS_BAD_TRIGGER,
        &valid_dialogue_v04(),
    ));
    assert!(
        diags.iter().any(|d| d.code == "DW0194"),
        "malformed trigger id must be DW0194: {diags:#?}"
    );
}

// ---------------------------------------------------------------------------
// DW0350 — a `use` trigger anchored where an NPC stands (round-6 island QA:
// two interaction hitboxes in one cell race for the same right-click, and the
// losing entity is silently dead — dialogue soft-lock class)
// ---------------------------------------------------------------------------

/// The trigger body decides the verdict: `use` on the keeper's own anchor is the
/// collision; `strike` there is the sanctioned form (the NPC's hitbox carries
/// the trigger's tag — no second entity, nothing to race).
fn quests_trigger_on_keeper_stand(on: &str) -> String {
    QUESTS_BAD_TRIGGER.replace(
        r#""id": "not-a-valid-trigger-id",
        "at": "anchor/exit",
        "on": { "on": "strike" },"#,
        &format!(
            r#""id": "trigger/nudge",
        "at": "anchor/keeper-stand",
        "on": {{ "on": "{on}" }},"#
        ),
    )
}

#[test]
fn use_trigger_on_an_npc_anchor_is_dw0350() {
    let quests = quests_trigger_on_keeper_stand("use");
    assert!(
        quests.contains("anchor/keeper-stand"),
        "fixture patch applied"
    );
    let diags = check_campaign(&campaign_with(
        &valid_npcs_v04(),
        &quests,
        &valid_dialogue_v04(),
    ));
    assert!(
        diags.iter().any(|d| d.code == "DW0350"),
        "a `use` trigger on an NPC's anchor must be DW0350: {diags:#?}"
    );
}

#[test]
fn strike_trigger_on_an_npc_anchor_is_not_dw0350() {
    let quests = quests_trigger_on_keeper_stand("strike");
    let diags = check_campaign(&campaign_with(
        &valid_npcs_v04(),
        &quests,
        &valid_dialogue_v04(),
    ));
    assert!(
        !diags.iter().any(|d| d.code == "DW0350"),
        "`strike` on an NPC's anchor is the sanctioned shared-hitbox form: {diags:#?}"
    );
}

// ---------------------------------------------------------------------------
// DW0195 — a `talk-to` targets an NPC despawned by a prerequisite quest
// ---------------------------------------------------------------------------

/// Two quests: `quest/open-the-door` despawns `npc/keeper` when its `obj/talk`
/// completes; `quest/second` (triggered by the first's completion, and the
/// declared finale) has its own `talk-to` on the now-despawned `npc/keeper`.
const QUEST_PLAN_TWO_QUESTS: &str = r#"{
  "dsl_version": "0.2.0",
  "campaign_id": "hello-world",
  "stage": "quest-plan",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "goal": "Get the Keeper to open the door and leave the keep.",
        "area": "area/keep",
        "npcs": ["npc/keeper"],
        "depends_on": [],
        "mandatory": true,
        "act": 1
      },
      {
        "id": "quest/second",
        "goal": "Speak to the Keeper once more.",
        "area": "area/keep",
        "npcs": ["npc/keeper"],
        "depends_on": ["quest/open-the-door"],
        "mandatory": true,
        "act": 1
      }
    ],
    "finale": "quest/second"
  }
}"#;

const QUESTS_DESPAWNED_REF: &str = r#"{
  "dsl_version": "0.4.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "despawn-npc", "npc": "npc/keeper" }
          ]
        },
        "on_complete": []
      },
      {
        "id": "quest/second",
        "trigger": { "type": "quest-complete", "quest": "quest/open-the-door" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk2", "npc": "npc/keeper" }
        ],
        "on_objective_complete": {},
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

/// The base dialogue tree, plus a third root option completing `obj/talk2` (so
/// `quest/second`'s objective has a reachable completing option too — DW0195 is
/// the deep despawn-ordering guard, not a dialogue-coverage gap).
const DIALOGUE_TWO_OBJECTIVES: &str = r#"{
  "dsl_version": "0.4.0",
  "campaign_id": "hello-world",
  "stage": "dialogue",
  "content": {
    "dialogues": [
      {
        "npc": "npc/keeper",
        "root": "dlg/greeting",
        "nodes": [
          {
            "id": "dlg/greeting",
            "text": "Halt, traveler. This keep is mine to guard, and the door stays shut.",
            "options": [
              { "label": "Who are you?", "next": "dlg/lore" },
              { "label": "Open the door, please.", "effects": [ { "type": "complete-objective", "objective": "obj/talk" } ] },
              { "label": "Anything else?", "effects": [ { "type": "complete-objective", "objective": "obj/talk2" } ] }
            ]
          },
          {
            "id": "dlg/lore",
            "text": "I am the Keeper. I have watched this gate since the moor swallowed the old road.",
            "options": [ { "label": "Back.", "next": "dlg/greeting" } ]
          }
        ]
      }
    ]
  }
}"#;

#[test]
fn talk_to_targets_despawned_npc_is_dw0195() {
    let raw = RawCampaign {
        world: common::read_valid("world.json"),
        npcs: valid_npcs_v04(),
        classes: common::read_valid("classes.json"),
        quest_plan: QUEST_PLAN_TWO_QUESTS.to_string(),
        quests: QUESTS_DESPAWNED_REF.to_string(),
        dialogue: DIALOGUE_TWO_OBJECTIVES.to_string(),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
    };
    let diags = check_campaign(&raw);
    assert!(
        diags.iter().any(|d| d.code == "DW0195"),
        "a talk-to on an npc despawned by a prerequisite quest must be DW0195: {diags:#?}"
    );
}
