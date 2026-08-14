//! The translation inventory reaches every effect root emission reaches
//! (task #168).
//!
//! `l10n::each_string` is the single traversal [`inventory`] and [`localize`]
//! share, so a player-visible string it does not visit is a string that is neither
//! demanded of a translator (`DW0180`) nor swapped at build time — it ships
//! **English-only in a translated build**, silently. It used to stop at three
//! effect roots where emission reaches five, so a `narrate` inside a
//! `traps[].payload` or a dialogue option's `set-checkpoint` `on_respawn` bundle
//! fell through exactly that hole.

mod common;

use std::collections::BTreeMap;

use delvewright_dsl::{
    Campaign, L10nDoc, L10nKind, RawCampaign, l10n_inventory, localize, on_screen_narrates,
    parse_campaign, validate_l10n,
};

/// The trap-payload narrate's inventory key (`fx.trap.<trap>.<i>`, keyed exactly
/// as an environment trigger's `fx.trig.<trigger>.<i>` is).
const TRAP_KEY: &str = "fx.trap.spring-the-door.0.narrate";
/// The dialogue-nested `on_respawn` narrate's key: the option's position, then the
/// `respawn` bundle segment every nested `set-checkpoint` already uses.
const RESPAWN_KEY: &str = "fx.dlg.keeper.greeting.1.1.respawn.0.narrate";

const TRAP_NARRATE_EN: &str = "The chest was a mouth, and it has closed.";
const RESPAWN_NARRATE_EN: &str = "You wake in the dark, and the keep remembers you.";

/// hello-world's `world` doc declaring one translation language.
fn world_doc() -> String {
    common::patch_doc(&common::read_valid("world.json"), |d| {
        d["content"]["languages"] = serde_json::json!(["zh-cn"]);
    })
}

/// hello-world's `quests` doc, with a raw `traps` array body spliced in.
fn quests_doc(traps: &str) -> String {
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "traps": [ {traps} ]
  }}
}}"#
    )
}

/// A trap whose command payload narrates — a player-visible line the redstone era
/// had no way to author, and spec-0022 made ordinary.
fn trap_with_narrate() -> String {
    format!(
        r#"{{
  "id": "trap/spring-the-door",
  "at": "anchor/exit",
  "trigger": "trapped-chest",
  "lethality": "harmful",
  "payload": [ {{ "type": "narrate", "style": "title", "text": "{TRAP_NARRATE_EN}" }} ]
}}"#
    )
}

/// hello-world's `dialogue` doc whose second option additionally sets a checkpoint
/// whose `on_respawn` bundle narrates. Everything else is the stock tree, so the
/// inventory differs from the stock campaign's by exactly this one key.
fn dialogue_doc() -> String {
    common::patch_doc(&common::read_valid("dialogue.json"), |d| {
        let opts = d["content"]["dialogues"][0]["nodes"][0]["options"]
            .as_array_mut()
            .expect("the greeting node has options");
        let effects = opts[1]["effects"]
            .as_array_mut()
            .expect("the second option completes the objective");
        assert_eq!(
            effects[0]["type"], "complete-objective",
            "the option this test hangs the checkpoint on has moved"
        );
        effects.push(serde_json::json!({
            "type": "set-checkpoint",
            "anchor": "anchor/exit",
            "on_respawn": [
                { "type": "narrate", "style": "title", "text": RESPAWN_NARRATE_EN }
            ]
        }));
    })
}

fn parse(quests: String, dialogue: String) -> Campaign {
    let raw = RawCampaign {
        world: world_doc(),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests,
        dialogue,
        world_edits: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

/// The campaign whose two narrates live in the newly-reached roots.
fn campaign_with_new_roots() -> Campaign {
    parse(quests_doc(&trap_with_narrate()), dialogue_doc())
}

/// The same campaign with neither narrate — its inventory is what a translator
/// working against the pre-fix traversal would have been handed.
fn campaign_without_new_roots() -> Campaign {
    parse(quests_doc(""), common::read_valid("dialogue.json"))
}

/// A complete `zh-cn` sidecar for `c`: every inventory key, translated by marking
/// it (the content is irrelevant here — coverage is what `DW0180` checks).
fn sidecar_for(c: &Campaign) -> BTreeMap<String, L10nDoc> {
    let content = l10n_inventory(c)
        .into_iter()
        .map(|(k, v)| (k, format!("[zh] {v}")))
        .collect();
    let doc = L10nDoc {
        dsl_version: "0.6.0".to_string(),
        campaign_id: c.world.campaign_id.clone(),
        kind: L10nKind::L10n,
        lang: "zh-cn".to_string(),
        content,
        source: Default::default(),
    };
    BTreeMap::from([("zh-cn".to_string(), doc)])
}

/// The control: a sidecar built from this very campaign's inventory covers it
/// exactly, in both directions. Guards the fixture itself — if this ever reds, the
/// two assertions below are measuring the wrong thing.
#[test]
fn a_complete_sidecar_is_clean() {
    let c = campaign_with_new_roots();
    let d = validate_l10n(&c, &sidecar_for(&c));
    assert!(
        d.is_empty(),
        "self-covering sidecar must validate clean: {d:#?}"
    );
}

/// A `narrate` in a `traps[].payload` and one in a dialogue option's
/// `set-checkpoint` `on_respawn` bundle are player-visible, so they are in the
/// inventory — and a sidecar that predates the widening is therefore **incomplete**
/// (`DW0180`, once per missing key).
///
/// Red against `origin/main`: the two keys are not inventoried, so the stale
/// sidecar covers the campaign "exactly" and nothing is reported — which is the
/// bug: the delve ships those two lines in English to a `zh-cn` player.
#[test]
fn dw0180_reds_a_sidecar_missing_the_new_roots() {
    let c = campaign_with_new_roots();
    let stale = sidecar_for(&campaign_without_new_roots());
    let d = validate_l10n(&c, &stale);
    let missing: Vec<&str> = d
        .iter()
        .filter(|x| x.code == "DW0180")
        .map(|x| x.message.as_str())
        .collect();
    assert_eq!(
        missing.len(),
        2,
        "exactly the two newly-reached strings are uncovered: {d:#?}"
    );
    assert!(
        missing.iter().any(|m| m.contains(TRAP_KEY)),
        "the trap payload's narrate must be demanded of the translator: {missing:#?}"
    );
    assert!(
        missing.iter().any(|m| m.contains(RESPAWN_KEY)),
        "…and so must the dialogue-nested one: {missing:#?}"
    );
}

/// Inventorying a key is only half the contract: [`localize`] walks the same
/// traversal, so the translated build must actually carry the translated line.
/// (`each_string` is the one traversal both go through — this pins that they
/// cannot drift for the two new roots.)
#[test]
fn the_new_roots_are_localized_too() {
    let mut c = campaign_with_new_roots();
    let inv = l10n_inventory(&c);
    assert_eq!(inv.get(TRAP_KEY).map(String::as_str), Some(TRAP_NARRATE_EN));
    assert_eq!(
        inv.get(RESPAWN_KEY).map(String::as_str),
        Some(RESPAWN_NARRATE_EN)
    );

    let translations = BTreeMap::from([
        (TRAP_KEY.to_string(), "[zh] trap".to_string()),
        (RESPAWN_KEY.to_string(), "[zh] respawn".to_string()),
    ]);
    localize(&mut c, &translations);
    let after = l10n_inventory(&c);
    assert_eq!(after.get(TRAP_KEY).map(String::as_str), Some("[zh] trap"));
    assert_eq!(
        after.get(RESPAWN_KEY).map(String::as_str),
        Some("[zh] respawn")
    );
}

/// The inventory ([`each_string`]) and the consumer scan the glyph/text-fit checks
/// walk (`on_screen_narrates` and its siblings) enumerate the **same** five roots.
/// They are hand-written mirrors — one mutable, one not — so this pins that they
/// agree: every string the checks measure is a string the inventory demands a
/// translation for, on a campaign that exercises all five roots at once.
#[test]
fn the_consumer_scan_and_the_inventory_agree_on_every_root() {
    let c = campaign_with_new_roots();
    let inv = l10n_inventory(&c);
    let narrates = on_screen_narrates(&c);

    let trap = narrates
        .iter()
        .find(|n| n.key == TRAP_KEY)
        .expect("a trap payload's on-screen narrate must be measurable");
    assert_eq!(trap.stage, "quests");
    assert_eq!(trap.path, "/content/traps/0/payload/0/text");

    let respawn = narrates
        .iter()
        .find(|n| n.key == RESPAWN_KEY)
        .expect("a dialogue-nested on_respawn narrate must be measurable");
    assert_eq!(
        respawn.stage, "dialogue",
        "…and reported at its real stage, not mislabelled `quests`"
    );
    assert_eq!(
        respawn.path,
        "/content/dialogues/0/nodes/0/options/1/effects/1/on_respawn/0/text"
    );

    for n in &narrates {
        assert!(
            inv.contains_key(&n.key),
            "`{}` is measured but never inventoried — the two walks have drifted",
            n.key
        );
    }
}
