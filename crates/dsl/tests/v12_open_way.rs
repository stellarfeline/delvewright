//! DSL v0.12 (spec-0042): the `open-way` effect — the version fence, the gate it
//! carries like every other gate consumer, and the fields it deliberately does
//! not have.
//!
//! What is NOT here: the way's geometry, its block or its direction. None of the
//! three is a campaign-side claim — they are read from the carrying piece's
//! exported metadata — so nothing at this layer can state them and nothing at
//! this layer can be wrong about them. The compiler's own tests
//! (`crates/compiler/tests/open_way.rs`) are where the reference is resolved
//! against a placed world.

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
/// `effects` after the open-gate.
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
    }
}

fn codes(quests: String) -> Vec<String> {
    check_campaign(&raw(quests))
        .into_iter()
        .map(|d| d.code.to_string())
        .collect()
}

/// The effect, as a campaign writes it: a placed piece and one of its ways.
const OPEN_WAY: &str = r#"{ "type": "open-way",
     "piece": "prefab/hello-room", "way": "broken-flight" }"#;

/// **The fence, and it is the fence rather than serde** (spec-0042 §5): the same
/// document is clean at 0.12.0 and `DW0141` at 0.11.0, with the verb named.
#[test]
fn open_way_validates_at_012_and_is_reserved_below_it() {
    let at_012 = codes(quests_doc("0.12.0", OPEN_WAY));
    assert!(
        !at_012.iter().any(|c| c == "DW0141"),
        "0.12.0 must accept the verb it introduces: {at_012:?}"
    );
    let at_011 = check_campaign(&raw(quests_doc("0.11.0", OPEN_WAY)));
    let fenced: Vec<_> = at_011.iter().filter(|d| d.code == "DW0141").collect();
    assert_eq!(fenced.len(), 1, "{at_011:?}");
    // The refusal names the construct and the version an author must raise to —
    // a `DW0141` that says only "reserved" sends nobody anywhere.
    assert!(
        fenced[0].message.contains("open-way"),
        "{}",
        fenced[0].message
    );
    assert!(
        fenced[0].message.contains("0.12.0"),
        "{}",
        fenced[0].message
    );
}

/// The verb carries the one gate every gate consumer carries — all three fields,
/// and no fourth mechanism of its own.
#[test]
fn open_way_carries_the_whole_gate() {
    let gated = r#"{ "type": "open-way", "piece": "prefab/hello-room", "way": "w",
       "requires_flags": ["flag/keeper-spoke"],
       "forbids_flags": ["flag/keeper-spoke"],
       "requires_state": [] }"#;
    let c = parse_campaign(&raw(quests_doc("0.12.0", gated))).expect("it parses");
    let effects = &c.quests.content.quests[0]
        .on_objective_complete
        .iter()
        .find(|(k, _)| k.as_str() == "obj/talk")
        .expect("the fixture declares an obj/talk bundle")
        .1;
    let open = &effects[1];
    assert_eq!(open.verb(), "open-way");
    assert_eq!(open.requires_flags().len(), 1);
    assert_eq!(open.forbids_flags().len(), 1);
    assert!(open.requires_state().is_empty());
    // The reference, and nothing else, is what the effect carries.
    let (piece, way) = open.way_write().expect("it answers the way accessor");
    assert_eq!(piece.as_str(), "prefab/hello-room");
    assert_eq!(way, "w");
}

/// **One authority** (spec-0042 AC8): a region, a block or a direction on the
/// effect is not a field this document form has, so no campaign can state one and
/// nothing has to compare two claims about the same way.
#[test]
fn an_open_way_has_no_region_no_block_and_no_direction() {
    for extra in [
        r#""region": { "anchor": "anchor/exit", "extent": [1, 1, 1] }"#,
        r#""block": "minecraft:stone""#,
        r#""opens": "laid""#,
        r#""boxes": []"#,
    ] {
        let effect = format!(
            r#"{{ "type": "open-way", "piece": "prefab/hello-room", "way": "w", {extra} }}"#
        );
        let found = codes(quests_doc("0.12.0", &effect));
        assert!(
            found.iter().any(|c| c == "DW0100"),
            "`{extra}` was accepted or dropped rather than refused: {found:?}"
        );
    }
}
