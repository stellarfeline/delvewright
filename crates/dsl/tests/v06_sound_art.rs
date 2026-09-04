//! DSL v0.6 (spec-0014): the `play-sound` effect and the `narrate` `art` style
//! validate under `0.6.0` and are reserved (`DW0141`) earlier. Sound-event
//! validation (`DW0326`), the unsupported `play-sound at: actor` gate (`DW0335`), and
//! art-glyph coverage (`DW0328`) are compiler-side checks (see the compiler's
//! `v06` tests) — the DSL only carries the schema and the version gate.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A stage-5 `quests` document whose single quest fires a `play-sound` and an
/// `art`-styled `narrate` on completion. `{ver}` is substituted for the stage's
/// `dsl_version`.
fn quests_doc(ver: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{ver}",
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [
          {{ "type": "play-sound", "sound": "minecraft:ui.toast.challenge_complete",
             "at": {{ "at": "players" }} }},
          {{ "type": "narrate", "text": "VICTORY", "style": "art" }},
          {{ "type": "campaign-complete" }}
        ]
      }}
    ]
  }}
}}"#
    )
}

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

/// `play-sound` + `narrate style: art` validate clean under `dsl_version 0.6.0`.
#[test]
fn v06_surface_validates_clean() {
    let diags = check_campaign(&campaign_with_quests(&quests_doc("0.6.0")));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for the v0.6 surface, got: {diags:#?}"
    );
}

/// The same surface under a pre-0.6 quests version is reserved → `DW0141`, for
/// both the `play-sound` effect and the `narrate` `art` style.
#[test]
fn v06_surface_reserved_before_0_6() {
    let diags = check_campaign(&campaign_with_quests(&quests_doc("0.5.0")));
    let reserved: Vec<_> = diags.iter().filter(|d| d.code == "DW0141").collect();
    assert!(
        reserved.iter().any(|d| d.path.ends_with("/type")),
        "play-sound effect must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
    assert!(
        reserved.iter().any(|d| d.path.ends_with("/style")),
        "narrate `style: art` must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}
