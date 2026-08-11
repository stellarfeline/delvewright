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

/// `QUESTS_WALK` with a stage-5 actor of `entity` walking the same line, so a
/// fixture can put a chosen BODY on the route rather than only the keeper.
fn quests_with_actor(entity: &str) -> String {
    serde_json::json!({
        "dsl_version": "0.6.0",
        "campaign_id": "hello-world",
        "stage": "quests",
        "content": {
            "actors": [
                { "id": "actor/subject", "entity": entity, "name": "Subject",
                  "anchor": "anchor/keeper-stand" }
            ],
            "quests": [ {
                "id": "quest/open-the-door",
                "trigger": { "type": "campaign-start" },
                "objectives": [
                    { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
                    { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
                      "radius": 2, "after": ["obj/talk"] }
                ],
                "on_objective_complete": { "obj/talk": [
                    { "type": "open-gate", "anchor": "anchor/door" },
                    { "type": "spawn-actor", "actor": "actor/subject" },
                    { "type": "move-actor", "actor": "actor/subject",
                      "to_anchor": "anchor/exit" }
                ] },
                "on_complete": [ { "type": "campaign-complete" } ]
            } ]
        }
    })
    .to_string()
}

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

/// hello-world's npcs doc with the keeper's body and traversal declaration
/// chosen by the caller, re-fenced to v0.11 (spec-0033).
///
/// The declaration is fenced on the **npcs** document's own `dsl_version`, which
/// is why this fixture can adopt it without touching the quests stage — the
/// per-stage fence, exercised rather than asserted.
fn npcs_declaring(base_entity: &str, locomotion: Option<&str>) -> String {
    let mut doc: serde_json::Value = serde_json::from_str(&read_hw("npcs.json")).unwrap();
    doc["dsl_version"] = serde_json::json!("0.11.0");
    let npc = &mut doc["content"]["npcs"][0];
    npc["base_entity"] = serde_json::json!(base_entity);
    if let Some(l) = locomotion {
        npc["traversal"] = serde_json::json!({ "locomotion": l });
    }
    doc.to_string()
}

/// Build the fixture campaign with the keeper as the only walker; `Ok` carries
/// the advisory diagnostics.
fn build(edits: String) -> Result<Vec<Diagnostic>, BuildFailure> {
    build_with(QUESTS_WALK.to_string(), edits)
}

/// Build with a chosen npcs document — the stage-2 half of the traversal
/// declaration.
fn build_npcs(npcs: String, edits: String) -> Result<Vec<Diagnostic>, BuildFailure> {
    build_all(Some(npcs), QUESTS_WALK.to_string(), edits).map(|(_, w)| w)
}

/// …keeping the emitted tree, so a fixture can read the binding ledger.
fn build_npcs_gate(npcs: String, edits: String) -> serde_json::Value {
    let (out, _) = build_all(Some(npcs), QUESTS_WALK.to_string(), edits).expect("fixture builds");
    serde_json::from_slice(
        out.get("validation/traversal-gate.json")
            .expect("the traversal proof emits its binding ledger"),
    )
    .expect("the ledger is valid JSON")
}

/// The `(code, message)` of a coded build failure — every failure these fixtures
/// can provoke is one, and unwrapping it at each call site buried the assertion.
fn coded(err: BuildFailure) -> (&'static str, String) {
    match err {
        BuildFailure::Diagnostic { code, message } => (code.id(), message),
        other => panic!("expected a coded build diagnostic, got {other:?}"),
    }
}

/// The emitted `validation/traversal-gate.json` for a fixture that builds.
fn build_gate(edits: String) -> serde_json::Value {
    let (out, _) = build_out(QUESTS_WALK.to_string(), edits).expect("fixture builds");
    let raw = out
        .get("validation/traversal-gate.json")
        .expect("the traversal proof emits its binding ledger");
    serde_json::from_slice(raw).expect("the ledger is valid JSON")
}

/// Build with a chosen quests document, so a fixture can put a chosen BODY on
/// the route ([`quests_with_actor`]).
fn build_with(quests: String, edits: String) -> Result<Vec<Diagnostic>, BuildFailure> {
    build_out(quests, edits).map(|(_, warnings)| warnings)
}

/// Build with a chosen quests document, keeping the emitted output tree.
fn build_out(
    quests: String,
    edits: String,
) -> Result<(emit::BuildOutput, Vec<Diagnostic>), BuildFailure> {
    build_all(None, quests, edits)
}

/// Build with a chosen npcs AND quests document.
fn build_all(
    npcs: Option<String>,
    quests: String,
    edits: String,
) -> Result<(emit::BuildOutput, Vec<Diagnostic>), BuildFailure> {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: npcs.unwrap_or_else(|| read_hw("npcs.json")),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests,
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
    let (code, message) = coded(err);
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

/// **The owner's round-21 correction, as a red that turns green.** A flying body
/// may skip the *climbing/surmounting* checks; the *collision* check it must
/// still owe. A closed fence gate's leaf spans the full cell across one axis,
/// the planned route runs down the cell's centre line, and the puppet performs
/// no right-click — and none of those three facts changes because the body has
/// wings. An earlier draft exempted fliers from both rules with one early
/// `continue`, so exactly this fixture compiled green.
#[test]
fn a_flier_walked_through_a_closed_gate_is_still_dw0452() {
    for entity in ["minecraft:ghast", "minecraft:bat", "minecraft:phantom"] {
        // The classification is real — this is not passing because the body was
        // read as Ground.
        assert_eq!(
            Traversal::of_entity(entity).locomotion,
            Locomotion::Flier,
            "{entity} must be classified a flier for this fixture to mean anything"
        );
        let err = build_with(
            quests_with_actor(entity),
            barrier_line(
                "minecraft:oak_fence",
                "minecraft:oak_fence_gate[facing=north,open=false]",
                0,
            ),
        )
        .expect_err("wings are not hands: a flier owes the gate rule");
        let (code, message) = coded(err);
        assert_eq!(code, DW_TRAVERSAL_IMPOSSIBLE, "{entity}: {message}");
        assert!(message.contains(entity), "{entity}: {message}");
    }
}

/// …and the same flier IS excused the surmount advisory, so the exemption is
/// per rule rather than per body — pinned against the same fixture that flags a
/// sheep, so the silence is evidence rather than an absent code path.
#[test]
fn the_same_flier_is_excused_the_surmount_advisory() {
    let surmount = barrier_line("minecraft:cobblestone_wall", "minecraft:stone", 0);
    let ground = build_with(quests_with_actor("minecraft:sheep"), surmount.clone())
        .expect("surmounting is advisory");
    assert!(
        ground
            .iter()
            .any(|d| d.code == DW_BARRIER_SURMOUNTED && d.message.contains("actor/subject")),
        "the fixture must flag a WALKING body, or a flier's silence proves nothing: {ground:#?}"
    );
    let flying = build_with(quests_with_actor("minecraft:ghast"), surmount)
        .expect("surmounting is advisory");
    assert!(
        !flying
            .iter()
            .any(|d| d.code == DW_BARRIER_SURMOUNTED && d.message.contains("actor/subject")),
        "a flier makes no ground step-up: {flying:#?}"
    );
}

/// **The safety property, end to end.** `Traversal::of_entity`'s unknown-id
/// fallback is the claim the whole module rests on, and a unit test on the table
/// only proves the classification — not that the classification is *acted on*.
/// So: put a body the table has never heard of on a route through a closed gate
/// and require the build to stop. If a future edit ever exempts an unrecognised
/// entity, this reds.
///
/// `minecraft:mannequin` is not a hypothetical: it is the body every skinned NPC
/// in the shipped delves wears.
#[test]
fn an_entity_the_table_never_heard_of_is_still_checked() {
    for entity in ["minecraft:mannequin", "minecraft:breeze"] {
        let err = build_with(
            quests_with_actor(entity),
            barrier_line(
                "minecraft:oak_fence",
                "minecraft:oak_fence_gate[facing=north,open=false]",
                0,
            ),
        )
        .expect_err("an unrecognised body must be checked, not exempted");
        let (code, message) = coded(err);
        assert_eq!(code, DW_TRAVERSAL_IMPOSSIBLE, "{entity}: {message}");
        assert!(message.contains(entity), "{entity}: {message}");
    }
}

/// …and a **climber** is exempt from the advisory tier and from nothing else.
/// A spider crossing a wall line over a low course is correct — that is the
/// capability the per-entity model exists for — but a spider still cannot open a
/// fence gate, so the error tier holds it exactly like a sheep.
///
/// The silence is only evidence if the fixture can speak, so this first proves
/// the SAME fixture with a sheep in it does raise `DW0453` naming
/// `actor/subject`. Without that half, a spider "exempt" because the actor was
/// never routed at all would read exactly like a spider exempt by capability —
/// the vacuous green CLAUDE.md names.
#[test]
fn a_climber_is_exempt_from_the_advisory_tier_only() {
    let surmount = barrier_line("minecraft:cobblestone_wall", "minecraft:stone", 0);
    let ground = build_with(quests_with_actor("minecraft:sheep"), surmount.clone())
        .expect("surmounting is advisory");
    assert!(
        ground
            .iter()
            .any(|d| d.code == DW_BARRIER_SURMOUNTED && d.message.contains("actor/subject")),
        "the fixture must flag a WALKING body, or the spider's silence proves \
         nothing: {ground:#?}"
    );
    let warnings = build_with(quests_with_actor("minecraft:spider"), surmount)
        .expect("surmounting is advisory");
    assert!(
        !warnings
            .iter()
            .any(|d| { d.code == DW_BARRIER_SURMOUNTED && d.message.contains("actor/subject") }),
        "a spider going over a wall is correct: {warnings:#?}"
    );
    let err = build_with(
        quests_with_actor("minecraft:spider"),
        barrier_line(
            "minecraft:oak_fence",
            "minecraft:oak_fence_gate[facing=north,open=false]",
            0,
        ),
    )
    .expect_err("climbing is not gate-opening");
    let (code, message) = coded(err);
    assert_eq!(code, DW_TRAVERSAL_IMPOSSIBLE, "{message}");
}

// ---------------------------------------------------------------------------
// spec-0033 — the author's side: a declaration the build holds you to
// ---------------------------------------------------------------------------

/// A wall line the route must cross over its one full-cube course — the shape
/// that raises `DW0453` for a walking body.
fn surmountable_wall() -> String {
    barrier_line("minecraft:cobblestone_wall", "minecraft:stone", 0)
}

/// **The owner's ruling, as a red that turns green** (2026-08-09). A sheep-bodied
/// keeper that walks over a wall is a finding; the author who really means it to
/// climb declares so, and the finding is answered — by the declaration, on the
/// body, rather than by nobody.
///
/// Both halves in one test on purpose: the silence is only evidence if the same
/// fixture, one field different, speaks.
#[test]
fn declaring_a_climber_answers_the_surmount_advisory_it_earns() {
    let undeclared = build_npcs(npcs_declaring("minecraft:sheep", None), surmountable_wall())
        .expect("surmounting is advisory");
    let w = undeclared
        .iter()
        .find(|d| d.code == DW_BARRIER_SURMOUNTED)
        .unwrap_or_else(|| {
            panic!("an undeclared sheep that climbs is the finding: {undeclared:#?}")
        });
    assert!(
        w.message.contains("npc/keeper") && w.message.contains("traversal"),
        "the advisory must name the declaration as the other resolution: {}",
        w.message
    );
    let declared = build_npcs(
        npcs_declaring("minecraft:sheep", Some("climber")),
        surmountable_wall(),
    )
    .expect("a declared climber going over a wall is what a climber does");
    assert!(
        !declared.iter().any(|d| d.code == DW_BARRIER_SURMOUNTED),
        "the declaration answers the advisory: {declared:#?}"
    );
}

/// …and the ledger states what that declaration cost, so the exemption is
/// visible rather than inferred from an empty findings list (CLAUDE.md: a green
/// gate that binds to nothing is vacuous).
#[test]
fn the_ledger_states_what_the_declarations_claimed_and_what_they_waived() {
    let gate = build_npcs_gate(
        npcs_declaring("minecraft:sheep", Some("climber")),
        surmountable_wall(),
    );
    assert_eq!(gate["declared"]["bodies"], serde_json::json!(1), "{gate}");
    assert_eq!(
        gate["declared"]["exercised"],
        serde_json::json!(1),
        "{gate}"
    );
    assert_eq!(
        gate["declared"]["advisories_waived"],
        serde_json::json!(1),
        "a declaration that waived no advisory bought nothing: {gate}"
    );
    assert_eq!(
        gate["declared"]["by_class"]["climber"],
        serde_json::json!(1),
        "{gate}"
    );
    // …and the legs are counted under the class the proof actually used.
    assert_eq!(gate["legs_by_class"]["climber"], gate["legs"], "{gate}");
    // A campaign that declares nothing reports a zero binding on this axis, and
    // says so rather than omitting the block.
    let none = build_npcs_gate(npcs_declaring("minecraft:sheep", None), surmountable_wall());
    assert_eq!(none["declared"]["bodies"], serde_json::json!(0), "{none}");
}

/// **The direction that drifts, second half**: a declaration the world does not
/// support. The same declared climber over a wall line with a real opening —
/// no route of its ever goes over anything — is `DW0454`, not a free exemption.
///
/// This is the property that keeps the surface a declaration instead of an
/// opt-out: you may claim your sheep climbs, and the build then requires the
/// climb.
#[test]
fn a_declaration_the_world_never_exercises_is_dw0454() {
    let err = build_npcs(
        npcs_declaring("minecraft:sheep", Some("climber")),
        barrier_line("minecraft:cobblestone_wall", "minecraft:air", 0),
    )
    .expect_err("a claim nothing pays for is refused");
    let (code, message) = coded(err);
    assert_eq!(code, "DW0454", "{message}");
    assert!(
        message.contains("npc/keeper") && message.contains("INERT"),
        "the message must name the body and the verdict: {message}"
    );
    assert!(
        message.contains("/content/npcs/0/traversal"),
        "the message must name the declaration's path: {message}"
    );
    assert!(
        message.contains("goes OVER a barrier line"),
        "the message must name the move the class governs: {message}"
    );
}

/// …and so is a declaration that merely restates what the entity id already
/// implies. A `ground` villager is a ground body with or without the field, so
/// the field holds it to nothing.
#[test]
fn a_declaration_that_restates_the_species_is_dw0454() {
    let err = build_npcs(
        npcs_declaring("minecraft:villager", Some("ground")),
        surmountable_wall(),
    )
    .expect_err("restating the derived class buys nothing");
    let (code, message) = coded(err);
    assert_eq!(code, "DW0454", "{message}");
    assert!(
        message.contains("already a `ground`"),
        "the message must say WHY it is inert: {message}"
    );
}

/// **A declaration can never reach the error tier.** `DW0452` is a
/// collision-and-interaction question with no authorable exemption: a puppet
/// makes no right-click whatever its paperwork says. The declared climber that
/// silences the advisory tier in the fixture above walks into the same closed
/// gate and the build still stops.
#[test]
fn a_declared_climber_still_cannot_walk_through_a_closed_gate() {
    let err = build_npcs(
        npcs_declaring("minecraft:sheep", Some("climber")),
        barrier_line(
            "minecraft:oak_fence",
            "minecraft:oak_fence_gate[facing=north,open=false]",
            0,
        ),
    )
    .expect_err("no declaration opens a fence gate");
    let (code, message) = coded(err);
    assert_eq!(code, DW_TRAVERSAL_IMPOSSIBLE, "{message}");
    assert!(message.contains("npc/keeper"), "{message}");
}

/// The declaration is not a one-way weakening: it TIGHTENS just as well. A
/// spider is a derived `Climber` and silent over a wall; the author who wants a
/// ground-bound spider says so, and the same route earns the advisory.
///
/// The undeclared half runs first, or the advisory could be coming from
/// anywhere.
#[test]
fn declaring_ground_binds_a_spider_back_to_the_surmount_rule() {
    let silent = build_npcs(
        npcs_declaring("minecraft:spider", None),
        surmountable_wall(),
    )
    .expect("a spider over a wall is what a spider does");
    assert!(
        !silent.iter().any(|d| d.code == DW_BARRIER_SURMOUNTED),
        "a derived climber must be silent, or the declared half proves nothing: {silent:#?}"
    );
    let bound = build_npcs(
        npcs_declaring("minecraft:spider", Some("ground")),
        surmountable_wall(),
    )
    .expect("surmounting stays advisory");
    assert!(
        bound
            .iter()
            .any(|d| d.code == DW_BARRIER_SURMOUNTED && d.message.contains("npc/keeper")),
        "declaring `ground` must bind the body back to the rule: {bound:#?}"
    );
}

/// A declaration on a body that never walks is inert for a third reason, and the
/// message says which — the fix differs (give it a route, or drop the field).
#[test]
fn a_declaration_on_a_body_that_never_moves_is_dw0454() {
    let mut doc: serde_json::Value =
        serde_json::from_str(&npcs_declaring("minecraft:sheep", Some("climber"))).unwrap();
    // A second NPC that declares a traversal and is never the subject of a
    // `move-npc`. The keeper keeps its walk, so the fixture still binds.
    let mut extra = doc["content"]["npcs"][0].clone();
    extra["id"] = serde_json::json!("npc/statue");
    extra["anchor"] = serde_json::json!("anchor/exit");
    doc["content"]["npcs"]
        .as_array_mut()
        .unwrap()
        .push(extra.clone());
    doc["content"]["npcs"][0]
        .as_object_mut()
        .unwrap()
        .remove("traversal");
    let err = build_npcs(doc.to_string(), surmountable_wall())
        .expect_err("a body that never moves has no locomotion to declare");
    let (code, message) = coded(err);
    assert_eq!(code, "DW0454", "{message}");
    assert!(
        message.contains("npc/statue") && message.contains("walks no leg at all"),
        "the message must name the third shape: {message}"
    );
}

/// **The second consumer, exercised.** Traversal is a property of a body that
/// moves, so it is one shared type on the stage-2 NPC and the stage-5 actor —
/// and a surface built onto one class and left inert on its sibling is exactly
/// the defect CLAUDE.md names. So the actor half runs the whole shape too: the
/// same wall, the same claim, the same proof, on the other object class.
#[test]
fn the_stage_5_actor_carries_the_same_declaration_and_the_same_proof() {
    let declared = |l: Option<&str>| {
        let mut doc: serde_json::Value =
            serde_json::from_str(&quests_with_actor("minecraft:sheep")).unwrap();
        doc["dsl_version"] = serde_json::json!("0.11.0");
        if let Some(l) = l {
            doc["content"]["actors"][0]["traversal"] = serde_json::json!({ "locomotion": l });
        }
        doc.to_string()
    };
    let undeclared = build_with(declared(None), surmountable_wall()).expect("advisory tier");
    assert!(
        undeclared
            .iter()
            .any(|d| d.code == DW_BARRIER_SURMOUNTED && d.message.contains("actor/subject")),
        "the fixture must flag the ACTOR, or the declared half proves nothing: {undeclared:#?}"
    );
    let climber = build_with(declared(Some("climber")), surmountable_wall())
        .expect("a declared climber going over a wall is what a climber does");
    assert!(
        !climber
            .iter()
            .any(|d| d.code == DW_BARRIER_SURMOUNTED && d.message.contains("actor/subject")),
        "the declaration must reach the actor: {climber:#?}"
    );
    let err = build_with(
        declared(Some("climber")),
        barrier_line("minecraft:cobblestone_wall", "minecraft:air", 0),
    )
    .expect_err("an actor's claim is held to the same standard an npc's is");
    let (code, message) = coded(err);
    assert_eq!(code, "DW0454", "{message}");
    assert!(
        message.contains("actor/subject") && message.contains("/content/actors/0/traversal"),
        "{message}"
    );
}

/// The build's binding ledger is emitted and states a non-zero count for a
/// campaign that walks — the artifact CLAUDE.md's rule 1 asks for, checked here
/// rather than trusted.
#[test]
fn the_ledger_states_what_it_examined() {
    let gate = build_gate(barrier_line(
        "minecraft:cobblestone_wall",
        "minecraft:air",
        0,
    ));
    assert_eq!(gate["unbound"], serde_json::json!(false));
    assert!(gate["legs"].as_u64().unwrap() >= 1, "{gate}");
    assert!(gate["route_cells"].as_u64().unwrap() > 0, "{gate}");
    assert_eq!(gate["legs_by_class"]["ground"], gate["legs"]);
    assert_eq!(
        gate["rules"]["jump_reach"]["bound"],
        serde_json::json!(false)
    );
}
