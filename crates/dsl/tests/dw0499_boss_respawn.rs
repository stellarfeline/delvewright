//! Souls ruling 5/7: "stage bosses never respawn on rest". Without this check
//! nothing structurally stops an author
//! declaring `respawns_on_rest: true` on a `tier: boss` wave.
//! `waves[].tier` and `waves[].respawns_on_rest` are two fields on
//! the SAME [`Wave`] struct — the only place in the DSL a "boss" and a
//! "rest-respawn" declaration can ever land on one another. An [`Actor`] carries
//! `tier` too (spec-0023's "other shape an elite takes"), but has no
//! `respawns_on_rest` field at all: an actor is killed by hand, never by a
//! `kill` objective, and the bonfire re-seat machinery (spec-0016 §1) only ever
//! re-summons **waves** — so an actor-shaped boss is structurally incapable of
//! declaring the violation this rule polices. The tie the ruling is about is
//! therefore exactly `waves[].tier == "boss"` on a wave that also declares
//! `respawns_on_rest: true`.
//!
//! `DW0499`, error, `dsl::validate` (validation-tier, exit 1) — same family as
//! `DW0370`, which this rule is layered beside: a `bonfire`-reachable
//! `respawns_on_rest` wave is inert without a bonfire (`DW0370`) and forbidden
//! outright when it is also billed `boss` (`DW0499`), regardless of whether a
//! bonfire exists to fire the re-seat.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A v0.7 quests document with a bonfire and a wave that re-seats on rest,
/// billed `tier`. `{TIER}` is substituted per test.
const QUESTS_V07_TEMPLATE: &str = r#"{
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
          { "type": "kill", "id": "obj/slay", "wave": "wave/ambush", "after": ["obj/talk"] },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/slay"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "spawn-wave", "wave": "wave/ambush" },
            { "type": "bonfire", "anchor": "anchor/keeper-stand",
              "on_rest": [ { "type": "narrate", "text": "The fire steadies you.", "style": "chat" } ] }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "waves": [
      {
        "id": "wave/ambush",
        "anchor": "anchor/keeper-stand",
        "respawns_on_rest": true,
        "TIER_SLOT": true,
        "mobs": [ { "entity": "minecraft:zombie", "count": 1 } ]
      }
    ]
  }
}"#;

/// Splice a `tier` declaration (or none at all) into the template in place of
/// the `"TIER_SLOT": true,` placeholder.
fn quests_with_tier(tier: Option<&str>) -> String {
    let slot = match tier {
        Some(t) => format!("\"tier\": \"{t}\","),
        None => String::new(),
    };
    QUESTS_V07_TEMPLATE.replace("\"TIER_SLOT\": true,\n        ", &slot)
}

/// The hello-world classes doc with a `flask` kit entry spliced into every
/// class (v0.8) — a bonfire campaign owes the party one (`DW0476`), and this
/// file's rule is not what that check exists to prove.
fn classes_with_flask() -> String {
    let mut v: serde_json::Value =
        serde_json::from_str(&common::read_valid("classes.json")).unwrap();
    v["dsl_version"] = serde_json::json!("0.19.0");
    for class in v["content"]["classes"].as_array_mut().unwrap() {
        class["kit"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "item": "minecraft:bread", "count": 3, "flask": true
            }));
    }
    serde_json::to_string(&v).unwrap()
}

fn campaign_with_quests(quests: &str) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: classes_with_flask(),
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

/// **Red case.** A `tier: boss` wave declaring `respawns_on_rest: true` is
/// `DW0499` — souls ruling 5/7, "stage bosses never respawn on rest". A
/// rest-respawning boss re-fight breaks the retry economy the ruling protects.
#[test]
fn boss_tier_wave_with_rest_reseat_is_dw0499() {
    let quests = quests_with_tier(Some("boss"));
    let diags = check_campaign(&campaign_with_quests(&quests));
    assert!(
        diags.iter().any(|d| d.code == "DW0499"),
        "a boss-tier wave declaring respawns_on_rest must be DW0499: {diags:#?}"
    );
    let hit = diags.iter().find(|d| d.code == "DW0499").unwrap();
    assert_eq!(hit.path, "/content/waves/0/respawns_on_rest");

    // The message is load-bearing content, not incidental prose: it must name
    // the offending wave, cite the ruling by its stable phrase, and carry both
    // prescriptions (re-bill as elite, or drop the re-seat) — a message
    // regression should be as machine-visible as a code regression.
    assert!(
        hit.message.contains("wave/ambush"),
        "the message must name the offending wave: {}",
        hit.message
    );
    assert!(
        hit.message.contains("never respawn on rest"),
        "the message must cite souls ruling 5/7 by its stable phrase: {}",
        hit.message
    );
    assert!(
        hit.message.contains("respawns_on_rest"),
        "the message must carry the drop-the-reseat prescription: {}",
        hit.message
    );
    assert!(
        hit.message.contains("elite"),
        "the message must carry the re-bill-as-elite prescription: {}",
        hit.message
    );
}

/// **Control.** An `elite`-tier wave with `respawns_on_rest: true` stays
/// silent — only `boss` is the tier the rule covers, and elite fights are
/// re-seated souls encounters by design (spec-0016 §1's whole contract).
#[test]
fn elite_tier_wave_with_rest_reseat_stays_silent() {
    let quests = quests_with_tier(Some("elite"));
    let diags = check_campaign(&campaign_with_quests(&quests));
    assert!(
        !diags.iter().any(|d| d.code == "DW0499"),
        "an elite-tier wave must not trip the boss-only rule: {diags:#?}"
    );
}

/// **Control.** An untiered (`ordinary`, absent) wave with
/// `respawns_on_rest: true` stays silent — the default tier carries no owner
/// ruling.
#[test]
fn untiered_wave_with_rest_reseat_stays_silent() {
    let quests = quests_with_tier(None);
    let diags = check_campaign(&campaign_with_quests(&quests));
    assert!(
        !diags.iter().any(|d| d.code == "DW0499"),
        "an untiered wave must not trip the boss-only rule: {diags:#?}"
    );
}

/// A `tier: boss` wave that does NOT re-seat on rest is untouched — the rule is
/// about the combination, never about billing a wave `boss` on its own.
#[test]
fn boss_tier_wave_without_rest_reseat_stays_silent() {
    let quests =
        quests_with_tier(Some("boss")).replace("\"respawns_on_rest\": true,\n        ", "");
    let diags = check_campaign(&campaign_with_quests(&quests));
    assert!(
        !diags.iter().any(|d| d.code == "DW0499"),
        "a boss-tier wave with no rest-reseat must not be DW0499: {diags:#?}"
    );
}
