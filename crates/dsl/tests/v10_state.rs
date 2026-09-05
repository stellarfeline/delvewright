//! DSL v0.10 (spec-0031): **runtime state** — a named, scoped, integer-valued
//! datum, the `set-state`/`add-state`/`clear-state` verbs that write one, and the
//! `requires_state` numeric comparison every gate consumer carries.
//!
//! What this file pins down:
//! * the whole surface validates clean, at
//!   every site — declaration, verb and comparison;
//! * a reference to an undeclared datum is `DW0500`;
//! * a datum a gate reads and nothing writes is `DW0501`, and a datum nothing
//!   reads is `DW0502` — the two halves of the vacuity ledger;
//! * a `player`-scoped datum touched where emission has no acting player is
//!   `DW0503`.
//!
//! The scope field is the reason a datum is *declared* at all, unlike a flag:
//! "each player has their own purse" and "the party shares one" are the same
//! JSON everywhere else, and the difference decides the whole design.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

fn campaign_with(quests: &str, dialogue: Option<&str>) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: dialogue
            .map(str::to_string)
            .unwrap_or_else(|| common::read_valid("dialogue.json")),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    }
}

fn codes(raw: &RawCampaign) -> Vec<String> {
    check_campaign(raw)
        .into_iter()
        .map(|d| d.code)
        .collect::<Vec<_>>()
}

/// A quests document that declares two data, writes them with all three verbs,
/// and reads them from an objective gate, an effect gate and a trigger gate.
fn quests_doc(version: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{version}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "state": [
      {{ "id": "state/toll", "scope": "party", "initial": 3,
        "note": "coins the gatekeeper still wants" }},
      {{ "id": "state/ride", "scope": "party" }}
    ],
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2,
            "after": ["obj/talk"],
            "requires_state": [ {{ "state": "state/toll", "op": "at-most", "value": 0 }} ] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [
            {{ "type": "add-state", "state": "state/toll", "amount": -1 }},
            {{ "type": "set-state", "state": "state/ride", "value": 1 }},
            {{ "type": "open-gate", "anchor": "anchor/door",
              "requires_state": [ {{ "state": "state/ride", "op": "equals", "value": 1 }} ] }},
            {{ "type": "clear-state", "state": "state/ride" }}
          ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "triggers": [
      {{
        "id": "trigger/toll-box",
        "at": "anchor/exit",
        "on": {{ "on": "strike" }},
        "requires_state": [ {{ "state": "state/toll", "op": "at-least", "value": 1 }} ],
        "effects": [ {{ "type": "add-state", "state": "state/toll", "amount": -1 }} ]
      }}
    ]
  }}
}}"#
    )
}

/// The whole v0.10 surface validates clean at `0.10.0`.
#[test]
fn runtime_state_validates_clean() {
    let raw = campaign_with(&quests_doc("0.19.0"), None);
    let d = check_campaign(&raw);
    assert!(d.is_empty(), "expected clean, got: {d:#?}");
}

/// `DW0500`: a comparison, and a verb, naming a datum nobody declared.
#[test]
fn an_undeclared_datum_is_rejected() {
    let doc = quests_doc("0.19.0").replace(
        r#""state": "state/ride", "value": 1"#,
        r#""state": "state/purse", "value": 1"#,
    );
    let raw = campaign_with(&doc, None);
    assert!(
        codes(&raw).contains(&"DW0500".to_string()),
        "a write to an undeclared datum is DW0500: {:#?}",
        check_campaign(&raw)
    );

    let doc = quests_doc("0.19.0").replace(
        r#"{ "state": "state/toll", "op": "at-most", "value": 0 }"#,
        r#"{ "state": "state/purse", "op": "at-most", "value": 0 }"#,
    );
    let raw = campaign_with(&doc, None);
    assert!(
        codes(&raw).contains(&"DW0500".to_string()),
        "a comparison against an undeclared datum is DW0500: {:#?}",
        check_campaign(&raw)
    );
}

/// `DW0501`: a gate reads a datum that no verb anywhere ever writes, so the
/// comparison's answer was decided when the campaign was written.
#[test]
fn a_datum_read_but_never_written_is_rejected() {
    // Drop both writes of `state/toll`, leaving the objective gate and the
    // trigger gate reading a datum frozen at its initial.
    let doc = quests_doc("0.19.0")
        .replace(
            r#"{ "type": "add-state", "state": "state/toll", "amount": -1 },
"#,
            "",
        )
        .replace(
            r#""effects": [ { "type": "add-state", "state": "state/toll", "amount": -1 } ]"#,
            r#""effects": [ { "type": "narrate", "text": "The box is empty." } ]"#,
        );
    let d = check_campaign(&campaign_with(&doc, None));
    let hit = d.iter().find(|x| x.code == "DW0501");
    assert!(hit.is_some(), "expected DW0501, got: {d:#?}");
    assert!(
        hit.unwrap().message.contains("state/toll"),
        "the diagnostic names the datum: {hit:#?}"
    );
}

/// `DW0502`: a datum nothing reads — the write is inert, or the declaration is
/// dead. Both cases, since both are silent.
#[test]
fn a_datum_never_read_is_rejected() {
    // (a) written, never read: drop the objective gate that reads `state/toll`.
    let doc = quests_doc("0.19.0")
        .replace(
            r#",
            "requires_state": [ { "state": "state/toll", "op": "at-most", "value": 0 } ] }"#,
            " }",
        )
        .replace(
            r#""requires_state": [ { "state": "state/toll", "op": "at-least", "value": 1 } ],"#,
            "",
        );
    let d = check_campaign(&campaign_with(&doc, None));
    assert!(
        d.iter()
            .any(|x| x.code == "DW0502" && x.message.contains("nothing ever asks")),
        "a written-but-unread datum is DW0502: {d:#?}"
    );

    // (b) declared and never touched at all.
    let doc = quests_doc("0.19.0").replace(
        r#"{ "id": "state/ride", "scope": "party" }"#,
        r#"{ "id": "state/ride", "scope": "party" },
      { "id": "state/dust", "scope": "party" }"#,
    );
    let d = check_campaign(&campaign_with(&doc, None));
    assert!(
        d.iter()
            .any(|x| x.code == "DW0502" && x.message.contains("state/dust")),
        "a dead declaration is DW0502: {d:#?}"
    );
}

/// `DW0503`: a `player`-scoped datum where emission has no acting player — read
/// by an objective's (party-evaluated) activation guard, and written from a
/// scheduler-only `sequence` step.
#[test]
fn a_player_scoped_datum_needs_an_acting_player() {
    let doc = quests_doc("0.19.0").replace(
        r#"{ "id": "state/toll", "scope": "party", "initial": 3,"#,
        r#"{ "id": "state/toll", "scope": "player", "initial": 3,"#,
    );
    let d = check_campaign(&campaign_with(&doc, None));
    assert!(
        d.iter()
            .any(|x| x.code == "DW0503" && x.message.contains("objective")),
        "an objective's guard is a party predicate: {d:#?}"
    );

    // Root R3: a TRIGGER's effects are polled on the tick with no executor, so a
    // per-player datum read or written there has no subject either. Three of the
    // seven roots are like this (R3, R4, R6) and four are not — the answer is
    // `EffectRootKind::runs_with_acting_player`'s, not this check's.
    let doc = quests_doc("0.19.0")
        .replace(
            r#"{ "id": "state/ride", "scope": "party" }"#,
            r#"{ "id": "state/ride", "scope": "player" }"#,
        )
        .replace(
            r#""effects": [ {{ "type": "add-state", "state": "state/toll", "amount": -1 }} ]"#
                .replace("{{", "{")
                .replace("}}", "}")
                .as_str(),
            r#""effects": [ { "type": "set-state", "state": "state/ride", "value": 1 } ]"#,
        );
    let d = check_campaign(&campaign_with(&doc, None));
    assert!(
        d.iter()
            .any(|x| x.code == "DW0503" && x.message.contains("no acting player")),
        "a trigger's effects run with no acting player: {d:#?}"
    );

    // Root R6: a shortcut's `on_unlock` is emitted `Audience::Scheduled` too, and
    // the READ side fails the same way as the write side.
    let doc = quests_doc("0.19.0")
        .replace(
            r#"{ "id": "state/ride", "scope": "party" }"#,
            r#"{ "id": "state/ride", "scope": "player" }"#,
        )
        .replace(
            r#"    "quests": ["#,
            r#"    "shortcuts": [
      { "id": "shortcut/back-way", "gate": "anchor/door", "unlock": "anchor/exit",
        "on_unlock": [
          { "type": "narrate", "text": "The bar lifts for good.",
            "requires_state": [ { "state": "state/ride", "op": "equals", "value": 1 } ] }
        ] }
    ],
    "quests": ["#,
        );
    let d = check_campaign(&campaign_with(&doc, None));
    assert!(
        d.iter().any(|x| x.code == "DW0503"
            && x.message.contains("requires_state")
            && x.message.contains("state/ride")),
        "a comparison inside `shortcuts[].on_unlock` has no acting player: {d:#?}"
    );

    // …and the four roots that DO have one are clean: `on_death` is the dying
    // player's own beat, so a per-player write there is exactly right.
    let doc = quests_doc("0.19.0")
        .replace(
            r#"{ "id": "state/ride", "scope": "party" }"#,
            r#"{ "id": "state/ride", "scope": "player" }"#,
        )
        .replace(
            r#"    "quests": ["#,
            r#"    "on_death": [ { "type": "set-state", "state": "state/ride", "value": 0 } ],
    "quests": ["#,
        );
    let d = check_campaign(&campaign_with(&doc, None));
    assert!(
        !d.iter().any(|x| x.code == "DW0503"),
        "`on_death` runs as the dying player, so a per-player write is legal: {d:#?}"
    );

    // The scheduler seam: a `sequence` step writes a per-player datum with no
    // player to write it to — the same seam `DW0357` polices for `carrier: one`.
    let doc = quests_doc("0.19.0")
        .replace(
            r#"{ "id": "state/ride", "scope": "party" }"#,
            r#"{ "id": "state/ride", "scope": "player" }"#,
        )
        .replace(
            r#"{ "type": "set-state", "state": "state/ride", "value": 1 },"#,
            r#"{ "type": "sequence", "steps": [ { "at_ticks": 0, "effects": [
              { "type": "set-state", "state": "state/ride", "value": 1 } ] } ] },"#,
        );
    let d = check_campaign(&campaign_with(&doc, None));
    assert!(
        d.iter()
            .any(|x| x.code == "DW0503" && x.message.contains("`sequence` step")),
        "a scheduled write of a per-player datum is DW0503: {d:#?}"
    );
}

/// A datum declared twice is `DW0111`, and a malformed id is `DW0110` — a datum
/// is a declared id like every other, and its scope has to be a single fact.
#[test]
fn datum_ids_follow_the_ordinary_id_rules() {
    let doc = quests_doc("0.19.0").replace(
        r#"{ "id": "state/ride", "scope": "party" }"#,
        r#"{ "id": "state/ride", "scope": "party" },
      { "id": "state/ride", "scope": "player" }"#,
    );
    assert!(
        codes(&campaign_with(&doc, None)).contains(&"DW0111".to_string()),
        "a datum declared twice is DW0111"
    );

    let doc = quests_doc("0.19.0").replace(r#""id": "state/ride""#, r#""id": "state/Ride""#);
    assert!(
        codes(&campaign_with(&doc, None)).contains(&"DW0110".to_string()),
        "a malformed datum id is DW0110"
    );
}

/// Two effects that differ **only** in their numeric gate are different effects,
/// so they must render differently — the `Debug` rendering names generated
/// `seq_<hash>` functions, and a collision there would silently give two
/// sequences one function.
#[test]
fn the_numeric_gate_is_part_of_an_effects_content_key() {
    use delvewright_dsl::{CompareOp, FlagId, QuestEffect, StateCompare, StateId};
    let bare = QuestEffect::SetFlag {
        flag: FlagId("flag/lit".to_string()),
        requires_flags: Vec::new(),
        forbids_flags: Vec::new(),
        requires_state: Vec::new(),
    };
    let gated = QuestEffect::SetFlag {
        flag: FlagId("flag/lit".to_string()),
        requires_flags: Vec::new(),
        forbids_flags: Vec::new(),
        requires_state: vec![StateCompare {
            state: StateId("state/toll".to_string()),
            op: CompareOp::AtLeast,
            value: 1,
        }],
    };
    // An UNGATED effect renders exactly as it did before v0.10 existed: that is
    // what keeps every existing campaign's `seq_<hash>` names where they are.
    assert_eq!(
        format!("{bare:?}"),
        r#"SetFlag { flag: FlagId("flag/lit"), requires_flags: [] }"#
    );
    assert_ne!(format!("{gated:?}"), format!("{bare:?}"));
}

/// **The gate walk inherits every effect root, including the two `on_death`
/// added after this surface was written.**
///
/// `for_each_gate`'s effect branch is defined on `stages::for_each_campaign_effect`,
/// which is defined on `effects::for_each_effect_root` — the single enumeration.
/// So a numeric gate on an effect inside the campaign's `on_death` bundle (root
/// R7) or inside a `shortcuts[].on_unlock` bundle (root R6) is reached with no
/// edit to `dsl::gate` at all. That is a claim about a walk, and a claim about a
/// walk is worth exactly what it is bound to — so it is proven rather than
/// asserted: an undeclared datum named from inside each of those two bundles must
/// raise `DW0500`, which can only happen if the walk got there.
///
/// This is the shape the `for_each_effect_root` family exists to prevent: a
/// proof written over "every gate" that silently stops at the
/// roots its author happened to know about.
#[test]
fn a_gate_inside_the_newest_effect_roots_is_still_walked() {
    // R7 — the campaign-wide `on_death` bundle.
    let doc = quests_doc("0.19.0").replace(
        r#"    "quests": ["#,
        r#"    "on_death": [
      { "type": "narrate", "text": "The toll goes back to the mud.",
        "requires_state": [ { "state": "state/unwalked-r7", "op": "at-least", "value": 1 } ] }
    ],
    "quests": ["#,
    );
    let d = check_campaign(&campaign_with(&doc, None));
    assert!(
        d.iter()
            .any(|x| x.code == "DW0500" && x.message.contains("state/unwalked-r7")),
        "a gate inside `on_death` (root R7) must be walked: {d:#?}"
    );

    // R6 — a shortcut's `on_unlock` bundle.
    let doc = quests_doc("0.19.0").replace(
        r#"    "quests": ["#,
        r#"    "shortcuts": [
      { "id": "shortcut/back-way", "gate": "anchor/door", "unlock": "anchor/exit",
        "on_unlock": [
          { "type": "narrate", "text": "The bar lifts for good.",
            "requires_state": [ { "state": "state/unwalked-r6", "op": "at-least", "value": 1 } ] }
        ] }
    ],
    "quests": ["#,
    );
    let d = check_campaign(&campaign_with(&doc, None));
    assert!(
        d.iter()
            .any(|x| x.code == "DW0500" && x.message.contains("state/unwalked-r6")),
        "a gate inside `shortcuts[].on_unlock` (root R6) must be walked: {d:#?}"
    );
}
