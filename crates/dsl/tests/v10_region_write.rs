//! DSL v0.10 region writes (spec-0031): `fill-region` / `clear-region` — the
//! version fence, the block registry, the anchor reference, and the gate the verbs
//! carry like every other gate consumer.
//!
//! What is deliberately NOT here: a second copy of the checks these verbs inherit.
//! The block id is `set-block`'s `DW0193`, the box anchor is every effect's
//! `DW0142` reference scan, the numeric gate is the one gate — the point of moving
//! the capability to the region is that none of those needed re-deriving.

use delvewright_dsl::{RawCampaign, check_campaign, parse_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/valid/hello-world")
            .join(name),
    )
    .unwrap()
}

/// A hello-world `quests` doc at `version` whose `obj/talk` bundle carries
/// `effects` (a raw JSON array body, no surrounding brackets) after the open-gate.
fn quests_doc(version: &str, effects: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{version}",
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }}, {effects} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn raw(quests: String) -> RawCampaign {
    RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests,
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    }
}

/// The `obj/talk` bundle of the parsed fixture — the list every case here writes
/// its verbs into.
fn talk_effects(c: &delvewright_dsl::Campaign) -> &[delvewright_dsl::QuestEffect] {
    c.quests.content.quests[0]
        .on_objective_complete
        .iter()
        .find(|(k, _)| k.as_str() == "obj/talk")
        .expect("the fixture declares an obj/talk bundle")
        .1
}

fn codes(quests: String) -> Vec<String> {
    check_campaign(&raw(quests))
        .into_iter()
        .map(|d| d.code.to_string())
        .collect()
}

/// The canonical pair: a box named by an anchor and an extent, filled with a block
/// and cleared again. Nothing names a gate.
const FILL: &str = r#"{ "type": "fill-region",
     "region": { "anchor": "anchor/exit", "extent": [1, 2, 1] },
     "block": "minecraft:water" }"#;
const CLEAR: &str = r#"{ "type": "clear-region",
     "region": { "anchor": "anchor/exit", "extent": [1, 2, 1] } }"#;

/// Both verbs validate clean at 0.10.0 — the acceptance criterion in one test:
/// filling and clearing a declared region is expressible without naming a gate.
#[test]
fn a_region_write_validates_clean_without_naming_a_gate() {
    let d = check_campaign(&raw(quests_doc("0.19.0", &format!("{FILL}, {CLEAR}"))));
    assert!(d.is_empty(), "a v0.10 region write validates clean: {d:#?}");
}

/// A `fill-region` block id is checked against the pinned 1.21.11 registry — the
/// same `DW0193` `set-block` gets, because the registry belongs to "a block id an
/// author wrote" and not to either verb.
#[test]
fn unknown_fill_block_is_dw0193() {
    let body = r#"{ "type": "fill-region",
         "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] },
         "block": "minecraft:not_a_real_block" }"#;
    let c = codes(quests_doc("0.19.0", body));
    assert!(
        c.contains(&"DW0193".to_string()),
        "an unknown fill-region block must be DW0193: {c:?}"
    );
}

/// A typo'd `region/anchor` is a dangling reference (`DW0142`), not a silently
/// unwritten — and therefore vacuously proven — region. This is the reference scan
/// every anchor-bearing effect feeds; the verbs only had to declare their anchor.
#[test]
fn dangling_region_anchor_is_dw0142() {
    for body in [
        r#"{ "type": "fill-region",
             "region": { "anchor": "anchor/nowhere", "extent": [0, 0, 0] },
             "block": "minecraft:water" }"#,
        r#"{ "type": "clear-region",
             "region": { "anchor": "anchor/nowhere", "extent": [0, 0, 0] } }"#,
    ] {
        let c = codes(quests_doc("0.19.0", body));
        assert!(
            c.contains(&"DW0142".to_string()),
            "a dangling region anchor must be DW0142: {c:?}"
        );
    }
}

/// Both verbs carry the whole gate — flags, negative flags and the numeric
/// comparison — because they are gate consumers like every other gatable verb.
/// `gate_consumers.rs` asserts this from the type across all thirty sites; this
/// asserts it end-to-end through the parser for the two added here.
#[test]
fn a_region_write_carries_the_whole_gate() {
    let body = r#"{ "type": "fill-region",
         "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] },
         "block": "minecraft:water",
         "requires_flags": ["flag/lift-called"],
         "forbids_flags": ["flag/riding"],
         "requires_state": [ { "state": "state/floor", "op": "equals", "value": 1 } ] }"#;
    let c = parse_campaign(&raw(quests_doc("0.19.0", body))).expect("campaign parses");
    let eff = &talk_effects(&c)[1];
    assert_eq!(eff.verb(), "fill-region");
    assert_eq!(eff.requires_flags().len(), 1);
    assert_eq!(eff.forbids_flags().len(), 1);
    assert_eq!(eff.requires_state().len(), 1);
}

/// The accessor the capability belongs to answers for both verbs, and answers
/// `None` for the block on a clear — one shape, two spellings.
#[test]
fn region_write_accessor_answers_for_both_verbs() {
    let c = parse_campaign(&raw(quests_doc("0.19.0", &format!("{FILL}, {CLEAR}"))))
        .expect("campaign parses");
    let effs = talk_effects(&c);
    let (zone, block) = effs[1]
        .region_write()
        .expect("fill-region is a region write");
    assert_eq!(zone.anchor.as_str(), "anchor/exit");
    assert_eq!(block, Some("minecraft:water"));
    let (_, block) = effs[2]
        .region_write()
        .expect("clear-region is a region write");
    assert_eq!(block, None, "a clear writes air, which is spelled `None`");
    // And the gate verbs answer through the gate half of the same capability.
    assert_eq!(effs[0].gate_region_write().map(|(_, f)| f), Some(false));
    assert!(
        effs[0].region_write().is_none(),
        "a gate's box comes from the prefab, not from the effect"
    );
}
