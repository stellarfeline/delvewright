//! `DW0856` — an area's bare `prefab` must name a piece the library holds.
//!
//! `prefab_pool` has carried this obligation since v0.2 (`DW0161`). The bare
//! `prefab` arm did not, and the cost was not a missing message: it was a
//! missing message that **took a proof with it**.
//!
//! An area whose prefab the registry has never heard of contributes no anchor
//! set, and every per-area anchor check reads a missing set as *defer to the
//! compiler* and skips. So a campaign with a mistyped `prefab` is not merely
//! accepted — it is accepted **more readily than a correct one**, because the
//! anchor proof (`DW0142`) over every quest in that area now examines nothing
//! and passes. That is the unbound vacuity mode, reachable by one keystroke.
//!
//! The third test below is the one that matters. It pins the vacuity as a fact
//! rather than describing it: the same wrong anchor that earns `DW0142` under a
//! correct piece earns **nothing** under a mistyped one, so `DW0856` is the
//! only thing standing between that campaign and a clean bill of health. Delete
//! the refusal and that test does not merely lose a code — it goes silent.

mod common;

use delvewright_dsl::{Diagnostic, RawCampaign, check_campaign};

/// A world doc binding one area to `prefab`, verbatim.
fn world_with(prefab: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.19.0",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {{
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "areas": [
      {{ "id": "area/keep", "name": "The Keep", "prefab": "{prefab}" }}
    ]
  }}
}}"#
    )
}

/// A quests doc whose `collect` objective sits at `anchor` — the per-area
/// anchor proof's own surface (`anchor_resolves`), which is what goes vacuous.
fn quests_collecting_at(anchor: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.19.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "collect", "id": "obj/gather", "item": "minecraft:bread",
             "count": 1, "anchor": "{anchor}", "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn campaign(prefab: &str, anchor: &str) -> RawCampaign {
    RawCampaign {
        world: world_with(prefab),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: quests_collecting_at(anchor),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    }
}

fn codes(d: &[Diagnostic]) -> Vec<&str> {
    d.iter().map(|x| x.code.as_str()).collect()
}

fn has(d: &[Diagnostic], code: &str) -> bool {
    d.iter().any(|x| x.code == code)
}

/// The refusal itself, and it names both halves of the question a reader has:
/// which piece is missing, and which area asked for it.
#[test]
fn a_bare_prefab_the_library_does_not_hold_is_refused() {
    let d = check_campaign(&campaign("prefab/hello-rom", "anchor/exit"));
    assert!(has(&d, "DW0856"), "{:?}", codes(&d));
    let msg = &d.iter().find(|x| x.code == "DW0856").unwrap().message;
    assert!(msg.contains("prefab/hello-rom"), "{msg}");
    assert!(msg.contains("area/keep"), "{msg}");
}

/// And it does not fire on a correct name. A refusal that reds valid campaigns
/// is a different defect, not a stricter version of this one.
#[test]
fn a_bare_prefab_the_library_holds_is_not_refused() {
    let d = check_campaign(&campaign("prefab/hello-room", "anchor/exit"));
    assert!(d.is_empty(), "{:?}", codes(&d));
}

/// **The reason the refusal exists.** One campaign, one wrong anchor, two
/// spellings of the piece:
///
/// * correct piece — the anchor proof examines the piece's four declared
///   anchors, does not find `anchor/nowhere`, and refuses (`DW0142`);
/// * mistyped piece — the area contributes no set, the proof examines **zero**
///   anchors, and `DW0142` is not raised at all.
///
/// The second case is strictly less checked than the first, which is why it
/// cannot be allowed to be green. `DW0856` is what makes it red, and it is the
/// only thing that does: assert that, and assert the silence it stands in for.
#[test]
fn a_mistyped_piece_takes_the_anchor_proof_with_it() {
    let right = check_campaign(&campaign("prefab/hello-room", "anchor/nowhere"));
    assert!(has(&right, "DW0142"), "{:?}", codes(&right));
    assert!(!has(&right, "DW0856"), "{:?}", codes(&right));

    let wrong = check_campaign(&campaign("prefab/hello-rom", "anchor/nowhere"));
    // The vacuity, stated as a fact rather than as prose: the very anchor that
    // was refused a line above is now examined by nothing.
    assert!(
        !has(&wrong, "DW0142"),
        "the anchor proof is expected to go VACUOUS here — if it now fires, this \
         test's premise has changed and the comment above it is wrong: {:?}",
        codes(&wrong)
    );
    // ...so this is the whole of what stands between that campaign and silence.
    assert_eq!(codes(&wrong), vec!["DW0856"], "{:?}", codes(&wrong));
}
