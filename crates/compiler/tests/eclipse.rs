//! `DW0359` — an NPC body may not eclipse an interaction affordance.
//!
//! The owner's island round-7 finding, in its exact shape: `npc/polyphemus` (a
//! `minecraft:warden` mannequin, 0.9 × 2.9 blocks) stands on `anchor/fire-pit`,
//! and so do the `obj/harden` and `obj/blind` interact affordances. The giant's
//! body ray-picks first, the interaction entities behind it are never reached,
//! and `obj/harden` — a required objective — is unreachable: a campaign
//! soft-lock that every other proof passed.
//!
//! `DW0350` only ever saw `use` **triggers** on an NPC's anchor, symbolically
//! (same anchor name). These fixtures pin the geometric statement: bodies and
//! affordances are boxes with real sizes, and the boxes may not overlap.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::eclipse::DW_BODY_ECLIPSE;
use delvewright_compiler::emit::{self, BuildFailure};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Diagnostic, RawCampaign, Severity, parse_campaign};

/// The hello-room prefab's anchors: `anchor/keeper-stand` at local `[5, 1, 4]`,
/// `spawn` at `[5, 1, 2]` (two cells north) and `anchor/exit` at `[5, 1, 8]`
/// (four cells south). Everything below is built out of those three.
fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// The hello-world NPC document with `npc/keeper`'s body and standing anchor
/// swapped for the fixture's.
fn npcs_doc(base_entity: &str, anchor: &str) -> String {
    read_hw("npcs.json")
        .replace("\"minecraft:villager\"", &format!("\"{base_entity}\""))
        .replace("\"anchor/keeper-stand\"", &format!("\"{anchor}\""))
}

/// A quests document whose quest carries one **prop-less** `interact` objective
/// at `interact_anchor` — a bare `minecraft:interaction` affordance, no block, so
/// the fixture changes nothing about the world's geometry except which cell the
/// party must click.
fn quests_doc(interact_anchor: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.4.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "interact", "id": "obj/pull-the-lever", "anchor": "{interact_anchor}",
             "after": ["obj/talk"], "title": "Pull the Lever",
             "hint": "The lever the Keeper never touches." }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/pull-the-lever"] }}
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

/// Build the fixture campaign; `Ok` carries the advisory diagnostics.
fn build(
    base_entity: &str,
    npc_anchor: &str,
    interact_anchor: &str,
) -> Result<Vec<Diagnostic>, BuildFailure> {
    build_quests(base_entity, npc_anchor, &quests_doc(interact_anchor))
}

/// The same, with the quests document supplied whole.
fn build_quests(
    base_entity: &str,
    npc_anchor: &str,
    quests: &str,
) -> Result<Vec<Diagnostic>, BuildFailure> {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: npcs_doc(base_entity, npc_anchor),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    };
    let campaign = parse_campaign(&raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let tree = CommandTree::v1_21_11();
    emit::build_with_warnings(
        &plan,
        &BTreeMap::new(),
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .map(|(_, warnings)| warnings)
}

/// The red fixture — the island's exact shape: the NPC stands on the very anchor
/// the interact objective's affordance occupies. The build must stop.
#[test]
fn npc_on_the_interact_anchor_is_dw0359() {
    let err = build(
        "minecraft:warden",
        "anchor/keeper-stand",
        "anchor/keeper-stand",
    )
    .expect_err("an NPC standing on an interact affordance must fail the build");
    let BuildFailure::Diagnostic { code, message } = err else {
        panic!("expected a coded build diagnostic, got {err:?}");
    };
    assert_eq!(code, "DW0359", "{message}");
    assert!(
        message.contains("npc/keeper") && message.contains("obj/pull-the-lever"),
        "the message must name both entities: {message}"
    );
    assert!(
        message.contains("intangible"),
        "the message must forbid the intangible-NPC 'fix': {message}"
    );
}

/// A plain villager body is just as fatal — the rule is about the affordance
/// being unreachable, not about the body being large.
#[test]
fn even_a_villager_body_eclipses_its_own_cell() {
    let err = build(
        "minecraft:villager",
        "anchor/keeper-stand",
        "anchor/keeper-stand",
    )
    .expect_err("a co-located affordance is unreachable whatever the body");
    let BuildFailure::Diagnostic { code, .. } = err else {
        panic!("expected a coded build diagnostic");
    };
    assert_eq!(code, DW_BODY_ECLIPSE);
}

/// The green fixture: the affordance sits at `anchor/exit`, four cells from the
/// NPC — clear of both tiers, and the build produces no `DW0359` at all.
#[test]
fn an_npc_two_or_more_blocks_away_is_clean() {
    let warnings = build("minecraft:warden", "anchor/keeper-stand", "anchor/exit")
        .expect("a body four cells from the affordance must build");
    assert!(
        !warnings.iter().any(|d| d.code == "DW0359"),
        "clear separation must not even warn: {warnings:#?}"
    );
}

/// The warning tier: a 1.95-wide ravager on `spawn` reaches to within 0.525
/// blocks of the `anchor/keeper-stand` affordance two cells away — not
/// overlapping, but close enough to shadow the crosshair from some approach
/// angles. Advisory, so the build still succeeds.
#[test]
fn a_body_within_a_block_of_the_affordance_warns() {
    let warnings = build("minecraft:ravager", "spawn", "anchor/keeper-stand")
        .expect("crowding is advisory — the build must still succeed");
    let w = warnings
        .iter()
        .find(|d| d.code == "DW0359")
        .unwrap_or_else(|| panic!("expected a DW0359 warning, got {warnings:#?}"));
    assert_eq!(w.severity, Severity::Warning);
    assert!(
        w.message.contains("0.52 blocks clear"),
        "the warning must report the measured gap: {}",
        w.message
    );
}

/// The parked-body scope: an NPC who **walks away** on the very beat that arms
/// the affordance shares its cell for a few ticks and blocks nothing. A declared
/// anchor is only a starting mark for a body the campaign moves, and the
/// compiler will not invent a timeline to decide otherwise — so a moved body is
/// out of scope, silently.
#[test]
fn a_body_the_campaign_moves_is_out_of_scope() {
    let quests = quests_doc("anchor/keeper-stand").replace(
        r#""obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]"#,
        r#""obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" }
          ]"#,
    );
    assert!(quests.contains("move-npc"), "fixture patch applied");
    let warnings = build_quests("minecraft:warden", "anchor/keeper-stand", &quests)
        .expect("a walker's starting mark is not a parked body");
    assert!(
        !warnings.iter().any(|d| d.code == "DW0359"),
        "a moved body must raise neither tier: {warnings:#?}"
    );
}
