//! `DW0452`/`DW0453` — a walked leg may only contain moves its BODY can make.
//!
//! The owner's island round-21 findings, in their exact shapes.
//!
//! * **Through** (`DW0452`): the mountain pen's fence gate shipped
//!   `open=false`, and sixteen walked legs crossed it. Passing a closed fence
//!   gate is a right-click the *player* makes; a `move-npc`/`move-actor` puppet
//!   is a `tp` polyline that interacts with nothing, and no runtime verb ever
//!   opens a gate — so the flock walked through a barrier the owner herself had
//!   to squeeze around.
//! * **Over** (`DW0453`): the beach fold's ring is `cobblestone_wall` on two
//!   sides and full-cube `mossy_cobblestone` along the middle of the others, so
//!   the router hopped the low course and the flock left the pen over its wall
//!   instead of through its opening.
//!
//! Both fixtures build hello-world with a stage-7 edit script that lays a
//! barrier line across the keeper's walk, so the route has exactly one way
//! through and the fixture controls what that way is. Doing it through the
//! editor is deliberate (as in `clearance.rs`): it writes the geometry the
//! shipped delve actually gets.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_compiler::traversal::{
    DW_BARRIER_SURMOUNTED, DW_TRAVERSAL_IMPOSSIBLE, Locomotion, Traversal,
};
use delvewright_dsl::{Diagnostic, RawCampaign, Severity, parse_campaign};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// A v0.6 quests doc whose only staged motion is one `move-npc` from the
/// keeper's stand to the room's exit — a straight north-south walk the barrier
/// line below cuts across.
const QUESTS_WALK: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
            "radius": 2, "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

/// hello-world's dialogue, re-fenced to v0.6 so it parses beside `QUESTS_WALK`.
fn dialogue_v06() -> String {
    read_hw("dialogue.json").replacen("\"0.2.0\"", "\"0.6.0\"", 1)
}

/// A one-batch stage-7 script laying a barrier line across the keeper's walk at
/// `z + 3` from `anchor/keeper-stand`, room-wide, with `middle` at its centre
/// cell (offset `[0, dy, 3]`) — the one place a body can cross.
fn barrier_line(line: &str, middle: &str, middle_dy: i32) -> String {
    serde_json::json!({
        "dsl_version": "0.6.0",
        "campaign_id": "hello-world",
        "stage": "world-edits",
        "content": { "batches": [ {
            "id": "batch/traversal-fixture",
            "area": "area/keep",
            "note": "A room-wide barrier line across the keeper's walk, pierced at one cell.",
            "edits": [
                { "verb": "select", "name": "region/line", "shape": {
                    "kind": "box",
                    "frame": { "kind": "anchor-relative", "anchor": "anchor/keeper-stand" },
                    "min": [-4, 0, 3], "max": [4, 0, 3] } },
                { "verb": "fill", "region": "region/line", "recipe": {
                    "blocks": [ { "block": line, "weight": 1.0 } ] } },
                { "verb": "select", "name": "region/middle", "shape": {
                    "kind": "box",
                    "frame": { "kind": "anchor-relative", "anchor": "anchor/keeper-stand" },
                    "min": [0, middle_dy, 3], "max": [0, middle_dy, 3] } },
                { "verb": "fill", "region": "region/middle", "recipe": {
                    "blocks": [ { "block": middle, "weight": 1.0 } ] } }
            ]
        } ] }
    })
    .to_string()
}

/// Build the fixture campaign; `Ok` carries the advisory diagnostics.
fn build(edits: String) -> Result<Vec<Diagnostic>, BuildFailure> {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: QUESTS_WALK.to_string(),
        dialogue: dialogue_v06(),
        world_edits: Some(edits),
    };
    let campaign = parse_campaign(&raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
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

/// The island's finding B: the only way across the line is a **closed** fence
/// gate, so the walk's route crosses it — a right-click the puppet never makes.
#[test]
fn a_walk_through_a_closed_fence_gate_is_dw0452() {
    let err = build(barrier_line(
        "minecraft:oak_fence",
        "minecraft:oak_fence_gate[facing=north,open=false]",
        0,
    ))
    .expect_err("a puppet cannot open a fence gate");
    let BuildFailure::Diagnostic { code, message } = err else {
        panic!("expected a coded build diagnostic");
    };
    assert_eq!(code, DW_TRAVERSAL_IMPOSSIBLE, "{message}");
    assert!(
        message.contains("npc/keeper") && message.contains("CLOSED FENCE GATE"),
        "the message must name the body and what it walked through: {message}"
    );
    assert!(
        message.contains("right-click USE"),
        "the message must name the capability the route assumed: {message}"
    );
    assert!(
        message.contains("open=true"),
        "the message must state the fix: {message}"
    );
}

/// …and the identical line whose gate ships **open** is silent. An open fence
/// gate has no collision at all, so the same route is honest for puppet and
/// player alike — which is the prescription, proven rather than asserted.
///
/// It also pins the `write_cell` half of this PR: `open=true` written by a
/// stage-7 edit must reach the occupancy model. Before it did not, and this
/// fixture failed with `DW0452` on a gate the delve visibly ships open.
#[test]
fn the_same_gate_shipped_open_is_silent() {
    let warnings = build(barrier_line(
        "minecraft:oak_fence",
        "minecraft:oak_fence_gate[facing=north,open=true]",
        0,
    ))
    .expect("an open fence gate is a bare threshold");
    assert!(
        !warnings
            .iter()
            .any(|d| d.code == DW_TRAVERSAL_IMPOSSIBLE || d.code == DW_BARRIER_SURMOUNTED),
        "an open gate raises neither tier: {warnings:#?}"
    );
}

/// The island's finding A: the line is `cobblestone_wall` except for one
/// full-cube course the body simply steps over — up one side, down the other.
#[test]
fn a_walk_over_a_full_cube_course_of_a_wall_is_dw0453() {
    let warnings = build(barrier_line(
        "minecraft:cobblestone_wall",
        "minecraft:stone",
        0,
    ))
    .expect("surmounting is advisory — the build must still succeed");
    let w = warnings
        .iter()
        .find(|d| d.code == DW_BARRIER_SURMOUNTED)
        .unwrap_or_else(|| panic!("expected a DW0453 warning, got {warnings:#?}"));
    assert_eq!(w.severity, Severity::Warning);
    assert!(
        w.message.contains("npc/keeper") && w.message.contains("OVER a barrier line"),
        "the message must name the body and what it crossed: {}",
        w.message
    );
    assert!(
        w.message.contains("1.5-tall fence/wall cell"),
        "the message must name the barrier the course belongs to: {}",
        w.message
    );
}

/// …and the same line pierced by an ordinary **opening** is silent: the rule is
/// about crossing a barrier over its own course, never about walls existing.
#[test]
fn a_line_with_a_real_opening_is_silent() {
    let warnings = build(barrier_line(
        "minecraft:cobblestone_wall",
        "minecraft:air",
        0,
    ))
    .expect("a wall with a doorway is ordinary staging");
    assert!(
        !warnings
            .iter()
            .any(|d| d.code == DW_BARRIER_SURMOUNTED || d.code == DW_TRAVERSAL_IMPOSSIBLE),
        "a wall with a doorway must raise neither tier: {warnings:#?}"
    );
}

/// The capability model, at the level that matters to the owner's ruling:
/// spiders really do climb in vanilla, so the rule may not be "no body crosses a
/// wall". It is "no body makes a move THIS body cannot make", and the table says
/// which is which.
#[test]
fn capabilities_are_per_entity_not_global() {
    assert_eq!(
        Traversal::of_entity("minecraft:spider").locomotion,
        Locomotion::Climber,
        "a spider routed over a wall is correct and must not be flagged"
    );
    assert_eq!(
        Traversal::of_entity("minecraft:sheep").locomotion,
        Locomotion::Ground,
        "a sheep doing the same thing is the bug"
    );
    assert!(
        !Traversal::of_entity("minecraft:spider").opens_gates,
        "climbing is not gate-opening: a spider is still held by DW0452"
    );
}
