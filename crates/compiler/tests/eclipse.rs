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
use delvewright_compiler::eclipse::{DW_AFFORDANCE_CONTEST, DW_BODY_ECLIPSE};
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

// ---------------------------------------------------------------------------
// `DW0878` — two AFFORDANCES on one cell
// ---------------------------------------------------------------------------
//
// The pairing none of `DW0359`, `DW0422` or `DW0489` reaches, because each of
// those needs one side of its pair to be something else. The gallery shipped it:
// an `interact` objective and a `use` trigger on `anchor/pedestal`, two
// `minecraft:interaction` boxes at one cell, and `validation/bot-run.sh` failed
// at step 2 saying the crosshair could acquire neither.

/// The fixture's quests document with a `use` trigger added at `trigger_anchor`.
fn quests_with_trigger(interact_anchor: &str, trigger_anchor: &str) -> String {
    let doc = quests_doc(interact_anchor);
    let trigger = format!(
        r#"    "triggers": [
      {{ "id": "trigger/read-the-plaque", "at": "{trigger_anchor}",
         "on": {{ "on": "use" }}, "audience": "presser",
         "effects": [ {{ "type": "narrate", "style": "chat",
                        "text": "The plaque is worn smooth." }} ] }}
    ],
    "quests": ["#
    );
    let patched = doc.replace("    \"quests\": [", &trigger);
    assert_ne!(patched, doc, "the trigger patch must apply");
    patched
}

/// The red fixture, in the gallery's exact shape: an `interact` objective's
/// affordance and a click trigger's body on one anchor. The build must stop, and
/// it must name both owners — the pair IS the whole content of the bug.
#[test]
fn two_affordances_on_one_cell_is_dw0878() {
    let quests = quests_with_trigger("anchor/exit", "anchor/exit");
    let err = build_quests("minecraft:villager", "anchor/keeper-stand", &quests)
        .expect_err("two interaction boxes on one cell must fail the build");
    let BuildFailure::Diagnostic { code, message } = err else {
        panic!("expected a coded build diagnostic, got {err:?}");
    };
    assert_eq!(code, "DW0878", "{message}");
    assert!(
        message.contains("obj/pull-the-lever") && message.contains("trigger/read-the-plaque"),
        "the message must name both owners: {message}"
    );
    assert!(
        message.contains("anchor/exit") && message.contains("COINCIDENT"),
        "the message must name the anchor and the geometry: {message}"
    );
    assert!(
        message.contains("non-pickable"),
        "the message must forbid the non-pickable 'fix': {message}"
    );
}

/// The constant, and the tier it declares. `DW0878` is `ExitTier::Build` for the
/// same reason its two neighbours are: the compiler will not stand behind a tree
/// whose geometry is undecidable.
#[test]
fn the_affordance_contest_code_is_dw0878_at_build_tier() {
    assert_eq!(DW_AFFORDANCE_CONTEST, "DW0878");
    assert_eq!(
        DW_AFFORDANCE_CONTEST.exit_tier(),
        delvewright_dsl::ExitTier::Build
    );
}

/// The green fixture. One cell of separation is enough, because the predicate is
/// exact coincidence and nothing wider: two boxes a cell apart are entered by
/// any ray at different distances, so a player aims at whichever they meant.
/// Refusing those would be a false certainty.
#[test]
fn one_cell_of_separation_clears_the_contest() {
    let quests = quests_with_trigger("anchor/exit", "anchor/keeper-stand");
    let warnings = build_quests("minecraft:villager", "spawn", &quests)
        .expect("affordances on distinct cells must build");
    assert!(
        !warnings.iter().any(|d| d.code == "DW0878"),
        "separated affordances must not even warn: {warnings:#?}"
    );
}

/// **`DW0878`'s binding, stated and moved.** A campaign with one affordance has
/// no pair to test and passes for free, which from outside is the same silence
/// as a campaign whose affordances all stand clear — so the proof states the set
/// it examined. Asserting it is not enough: the count has to MOVE with the
/// objects, or a constant would satisfy it.
#[test]
fn the_affordance_contest_states_what_it_examined() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let binding_of = |quests: &str| {
        let campaign = parse_campaign(&RawCampaign {
            world: read_hw("world.json"),
            npcs: npcs_doc("minecraft:villager", "spawn"),
            classes: read_hw("classes.json"),
            quest_plan: read_hw("quest-plan.json"),
            quests: quests.to_string(),
            dialogue: read_hw("dialogue.json"),
            world_edits: None,
            geometry_brief: None,
            layout_graph: None,
            site_plan: None,
            detail_plan: None,
        })
        .expect("campaign parses");
        let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
        delvewright_compiler::eclipse::affordance_contest_binding(&plan)
            .into_iter()
            .map(|(kind, label, _cell)| format!("{kind} {label}"))
            .collect::<Vec<_>>()
    };
    let just_the_objective = binding_of(&quests_doc("anchor/exit"));
    let plus_the_trigger = binding_of(&quests_with_trigger("anchor/exit", "anchor/keeper-stand"));
    assert_eq!(
        just_the_objective.len(),
        1,
        "the objective's affordance alone: {just_the_objective:?}"
    );
    assert_eq!(
        plus_the_trigger.len(),
        2,
        "the trigger's body enters the same set: {plus_the_trigger:?}"
    );
    assert!(
        plus_the_trigger
            .iter()
            .any(|s| s.contains("trigger/read-the-plaque")),
        "the set names what it examined: {plus_the_trigger:?}"
    );
}
