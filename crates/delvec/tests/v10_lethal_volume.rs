//! DSL v0.10 lethal volumes (spec-0031): what the compiler emits, what the
//! completability proof refuses, and the runtime half the PackTest suite carries.
//!
//! The `hello-room` prefab is the fixture for every case here because its geometry
//! is a corridor with exactly one doorway (`anchor/door`, the gate region
//! `[4,1,6]..[5,3,6]`), which makes "the only route runs through the volume" a
//! two-line declaration rather than a synthetic world.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// A hello-world `quests` doc with the given `lethal_volumes` body and an optional
/// extra `on_objective_complete` effect for `obj/talk`.
fn quests_doc(volumes: &str, talk_effects: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.24.0",
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }}{talk_effects} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "lethal_volumes": [ {volumes} ]
  }}
}}"#
    )
}

/// Parse, then TAG the campaign's player-visible strings exactly as `delvec build`
/// does (`main.rs`, spec-0029 i18n v2). Without the tag every emitter lowers a
/// bare `{"text": …}` literal, so an untagged test would silently stop proving
/// that the death wording travels as a translatable component.
fn parse_hw(quests: &str) -> Campaign {
    parse_hw_with_edits(quests, None)
}

/// [`parse_hw`], with an optional stage-7 `world-edits` document.
///
/// Declaring one is what puts a campaign on `emit::build`'s **`edit_replay`
/// arm**, and that arm builds the navigation world itself instead of taking
/// `nav::World::from_plan`. Every proof below therefore has two arms to be true
/// on, and for as long as the arms applied different premises only one of them
/// was ever tested — see `a_volume_across_the_only_route_is_dw0510_under_edits`.
fn parse_hw_with_edits(quests: &str, world_edits: Option<&str>) -> Campaign {
    let raw = RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: hw("dialogue.json"),
        world_edits: world_edits.map(str::to_string),
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    let mut c = parse_campaign(&raw).expect("campaign parses");
    delvewright_dsl::tag_translatables(&mut c);
    c
}

/// The smallest `world-edits` document that puts a hello-world campaign on the
/// edit-replay arm: one batch that re-dresses a 2×2 patch of the keep's floor in
/// cobblestone. Both blocks are full cubes, so the geometry every proof reasons
/// over is untouched and the only thing this changes is WHICH ARM builds the
/// world.
const ONE_BATCH: &str = r#"{
  "dsl_version": "0.24.0",
  "campaign_id": "hello-world",
  "stage": "world-edits",
  "content": {
    "batches": [
      {
        "id": "batch/dress-a-corner",
        "area": "area/keep",
        "note": "a floor patch, so this campaign takes the edit-replay arm",
        "edits": [
          {
            "verb": "select",
            "name": "region/corner",
            "shape": {
              "kind": "box",
              "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
              "min": [1, 0, 1],
              "max": [2, 0, 2]
            }
          },
          {
            "verb": "replace",
            "region": "region/corner",
            "matching": ["minecraft:stone"],
            "recipe": { "blocks": [{ "block": "minecraft:cobblestone", "weight": 1.0 }] }
          }
        ]
      }
    ]
  }
}"#;

fn structures(plan: &Plan) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                out.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    out
}

fn try_build(c: &Campaign) -> Result<BuildOutput, emit::BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let s = structures(&plan);
    emit::build(
        &plan,
        &BTreeMap::new(),
        &s,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn build(c: &Campaign) -> BuildOutput {
    try_build(c).expect("build succeeds")
}

fn failure_code(c: &Campaign) -> String {
    match try_build(c) {
        Ok(_) => panic!("expected the build to fail"),
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            eprintln!("{code}: {message}");
            code.to_string()
        }
        Err(other) => panic!("expected a diagnostic failure, got {other:?}"),
    }
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| {
                panic!(
                    "`{path}` is emitted; have {:#?}",
                    out.keys().collect::<Vec<_>>()
                )
            })
            .clone(),
    )
    .unwrap()
}

/// **The burning floor at the road's edge** — the one cell of `anchor/exit`, over
/// a floor of molten stone (spec-0062 §8).
///
/// It is only legal with [`SIDE_DOOR`] carved AND its floor laid in magma, and
/// the two are one fixture for one reason. **A body is as wide as its hitbox**,
/// so the nine cells around this volume are cells no route may use; this room was
/// built with one 2-wide doorway that opens straight onto the drop's edge, so the
/// side door is what leaves the party a way through. And those nine cells are
/// floor the party can walk to, so under `DW0891` they may not read as ordinary
/// stone: the fixture lays magma under exactly them and declares it. Danger is
/// visible, or the engine refuses it.
const HARMLESS: &str = r#"{
  "id": "lethal/the-burn",
  "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] },
  "message": "The road ends at a floor of molten stone.",
  "damage_type": "fire",
  "shown_by": ["minecraft:magma_block"]
}"#;

/// A second way through the keep's dividing wall, at its west end.
///
/// `hello-room` has exactly one doorway, 2 cells wide, in the middle of the wall
/// — and `anchor/exit` is three cells beyond it. A volume kills on hitbox
/// intersection, so the cells that share a face with one are not footing, and a
/// one-cell drop at the exit anchor therefore seals the only door. That is a true
/// property of this room and not a defect in the rule, so the fixture gains the
/// geometry it needs rather than the rule being narrowed to fit it.
///
/// **West** rather than east on purpose: an endpoint snap picks the nearest
/// standable cell and breaks ties lexicographically, with no regard for whether
/// anything can walk to it, so `[3, 65, 8]` wins the tie over `[7, 65, 8]`. A
/// door on the east side leaves the route proof staring at a cell it cannot
/// reach.
const SIDE_DOOR: &str = r#"{
  "dsl_version": "0.24.0",
  "campaign_id": "hello-world",
  "stage": "world-edits",
  "content": {
    "batches": [
      {
        "id": "batch/the-burn-and-the-side-door",
        "area": "area/keep",
        "note": "ONE batch, and that is the fixture: every proof re-runs after every batch, so the floor of molten stone and the way past it have to arrive together — signalled last, the floor read as stone while the earlier batch was judged (DW0891); carved last, the route was shut while the burn was judged (DW0510)",
        "edits": [
          {
            "verb": "select",
            "name": "region/the-burn",
            "shape": {
              "kind": "box",
              "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
              "min": [4, 0, 7],
              "max": [6, 0, 9]
            }
          },
          {
            "verb": "replace",
            "region": "region/the-burn",
            "matching": ["minecraft:stone"],
            "recipe": { "blocks": [{ "block": "minecraft:magma_block", "weight": 1.0 }] }
          },
          {
            "verb": "select",
            "name": "region/side-door",
            "shape": {
              "kind": "box",
              "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
              "min": [2, 1, 6],
              "max": [2, 2, 6]
            }
          },
          {
            "verb": "replace",
            "region": "region/side-door",
            "matching": ["minecraft:stone"],
            "recipe": { "blocks": [{ "block": "minecraft:air", "weight": 1.0 }] }
          }
        ]
      }
    ]
  }
}"#;

// --- emission -------------------------------------------------------------

/// The volume emits a tick driver and the two-line body: players re-bound one at a
/// time so the wording reaches the one it is about, everything else in one
/// `/damage` minus the engine's own machinery.
#[test]
fn a_volume_emits_a_tick_driver_and_a_killing_body() {
    // On the exit cell, with the west door carved so the party can still reach
    // the objective past it ([`SIDE_DOOR`]): this test is about emission alone.
    let c = parse_hw_with_edits(
        &quests_doc(
            r#"{ "id": "lethal/pit",
             "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] },
             "message": "The pit takes you.", "damage_type": "fire",
             "shown_by": ["minecraft:magma_block"] }"#,
            "",
        ),
        Some(SIDE_DOOR),
    );
    let out = build(&c);
    let tick = text(&out, "datapack/data/hello-world/function/tick.mcfunction");
    assert!(
        tick.contains("function hello-world:lethal_pit"),
        "the volume is driven every tick: {tick}"
    );

    let body = text(
        &out,
        "datapack/data/hello-world/function/lethal_pit.mcfunction",
    );
    assert!(
        body.contains("execute as @a[") && body.contains("tag=!dw_cutscene"),
        "players are re-bound and a cutscene watcher is never harmed: {body}"
    );
    assert!(
        body.contains("run function hello-world:lethal_pit_kill"),
        "the per-player half runs the volume's own kill function: {body}"
    );
    // Every engine-machinery type is excluded from the entity sweep — a volume
    // over a cutscene dolly must not erase the camera.
    for t in [
        "type=!minecraft:interaction",
        "type=!minecraft:marker",
        "type=!minecraft:item_display",
        "type=!minecraft:block_display",
        "type=!minecraft:text_display",
        "type=!minecraft:player",
    ] {
        assert!(body.contains(t), "the entity sweep excludes {t}: {body}");
    }

    let kill = text(
        &out,
        "datapack/data/hello-world/function/lethal_pit_kill.mcfunction",
    );
    assert!(
        kill.contains("tellraw @s") && kill.contains("The pit takes you."),
        "the wording reaches the player who died: {kill}"
    );
    // The wording is a CONSEQUENCE of the blow, never a prediction of it, and
    // this is the assertion that pins it: PackTest cannot read chat, so the
    // runtime template can only show the guard's condition coming out false —
    // that the message is actually conditioned on it is checked here.
    //
    // Measured on the pinned 1.21.11 toolserver, after getting it wrong twice:
    // `/damage` reports SUCCESS while doing nothing (a respawned player is
    // invulnerable for 59 ticks), so `execute store success` is inert; the guard
    // must read the outcome. A dummy at `Health: 20f` in a swinging volume was
    // still at `Health: 20f` after 202 ticks with `/damage` answering success
    // every tick.
    assert!(
        kill.contains("execute if score #leth_hp dw.sys matches ..0 run tellraw @s"),
        "the wording is conditioned on the player actually ending up dead — an \
         unconditional `tellraw` prints once per tick for three seconds after any \
         respawn, about a death that is not happening: {kill}"
    );
    assert!(
        !kill.contains("store success"),
        "the guard must not read `/damage`'s own result, which reports success even \
         when it does nothing (measured): {kill}"
    );
    // The blow must come first; a guard evaluated before the damage reads the
    // health the player had on the way in.
    let dmg_at = kill.find("damage @s").expect("the blow is emitted");
    let guard_at = kill.find("execute if score").expect("the guard is emitted");
    assert!(
        dmg_at < guard_at,
        "the blow precedes the guard that reads its outcome: {kill}"
    );
    // Delivered as a text component, which is what makes it translatable AND
    // readable by a player who declined the resource pack (spec-0029 §3).
    assert!(
        kill.contains("\"translate\"") && kill.contains("\"fallback\""),
        "the wording is a translate+fallback component, not a bare literal: {kill}"
    );
    // The declared damage type words vanilla's own broadcast.
    assert!(
        kill.contains("damage @s 1000 minecraft:on_fire"),
        "the kill uses the declared damage type: {kill}"
    );
}

/// A campaign that declares no volume emits nothing new — no tick line, no
/// function, no ledger. This is the byte-identity claim in its smallest form.
#[test]
fn no_volume_emits_nothing() {
    let c = parse_hw(&quests_doc("", ""));
    let out = build(&c);
    assert!(
        !out.keys().any(|k| k.contains("lethal")),
        "a campaign with no lethal volume emits no lethal artifact: {:#?}",
        out.keys().collect::<Vec<_>>()
    );
}

// --- DW0510: the completability proof knows about it -----------------------

/// The volume across the doorway, and the ORDER two rules answer it in
/// (spec-0062 §4 and criterion 6).
const THRESHOLD: &str = r#"{
  "id": "lethal/the-threshold",
  "region": { "anchor": "anchor/door", "extent": [3, 3, 0] },
  "message": "The threshold burns."
}"#;

/// The same volume with the floor it catches laid in magma and declared.
const THRESHOLD_SIGNALLED: &str = r#"{
  "id": "lethal/the-threshold",
  "region": { "anchor": "anchor/door", "extent": [3, 3, 0] },
  "message": "The threshold burns.",
  "damage_type": "fire",
  "shown_by": ["minecraft:magma_block"]
}"#;

/// The floor course under [`THRESHOLD`]'s keep-out, in molten stone: the band
/// `z = 5..7` of the keep's floor, which covers every walked cell the volume
/// catches. Nothing else in the piece moves.
const BURNING_THRESHOLD: &str = r#"{
  "dsl_version": "0.24.0",
  "campaign_id": "hello-world",
  "stage": "world-edits",
  "content": {
    "batches": [
      {
        "id": "batch/burning-threshold",
        "area": "area/keep",
        "note": "the floor the threshold volume catches, in the block that shows it",
        "edits": [
          {
            "verb": "select",
            "name": "region/threshold-floor",
            "shape": {
              "kind": "box",
              "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
              "min": [0, 0, 5],
              "max": [8, 0, 7]
            }
          },
          {
            "verb": "replace",
            "region": "region/threshold-floor",
            "matching": ["minecraft:stone"],
            "recipe": { "blocks": [{ "block": "minecraft:magma_block", "weight": 1.0 }] }
          }
        ]
      }
    ]
  }
}"#;

/// **The order, as a property** (spec-0062 §4, criterion 6). The keep has exactly
/// one doorway; a volume across it both catches the floor the party walks and
/// closes the only route to the exit objective. Two rules are true about it, and
/// the one the build reports is the CAUSE: nothing in that room says it kills.
/// `DW0510` — the route closure — is its symptom, and an author sent to "give the
/// party a route around it" would be repairing the wrong thing.
#[test]
fn a_volume_that_catches_floor_and_closes_the_route_is_dw0891_not_dw0510() {
    let c = parse_hw(&quests_doc(THRESHOLD, ""));
    assert_eq!(failure_code(&c), "DW0891");
}

/// **The same volume with its floor signalled: now `DW0510`.** One thing moved —
/// the floor under the keep-out is molten stone and the volume declares it — and
/// the verdict moves with it. This is what makes the pair a statement about the
/// order rather than about which check happens to run first: a volume the player
/// can see is judged by the route proofs exactly as before.
#[test]
fn a_signalled_volume_across_the_only_route_is_dw0510() {
    let c = parse_hw_with_edits(
        &quests_doc(THRESHOLD_SIGNALLED, ""),
        Some(BURNING_THRESHOLD),
    );
    assert_eq!(failure_code(&c), "DW0510");
}

/// **The same volume, the other arm.** A campaign that declares `world-edits`
/// takes `emit::build`'s `edit_replay` arm, which builds the navigation world
/// from the EDITED bytes rather than through `nav::World::from_plan` — and for
/// as long as that arm assembled the world's premises by hand it applied the
/// ambient and the gate seals and not the lethal volumes. Every proof the volume
/// backs (`DW0510`, `DW0311`, the exported-route check) was therefore vacuous for
/// every edit-carrying campaign, including the engine's own gallery, whose bot
/// walked its critical path into a wither box and died there.
///
/// The edit is two-by-two of floor re-dressed in cobblestone: it changes nothing
/// a route can feel, so a difference in verdict between this test and its
/// no-edits twin above can only be the arm.
///
/// The code it now meets is `DW0891` rather than `DW0510`, and that is a
/// STRONGER statement of the same claim: the batch replay is a second entry point
/// for these proofs, and `DW0891` is asked there — before the route proof, as
/// spec-0062 §4 orders it — over the volumes this arm carries. A gate bound at
/// one of two doors is bound at neither.
#[test]
fn a_volume_across_the_only_route_is_refused_under_edits_too() {
    let c = parse_hw_with_edits(&quests_doc(THRESHOLD, ""), Some(ONE_BATCH));
    assert_eq!(failure_code(&c), "DW0891");
}

/// The **binding count** the same defect showed from the other side, and the
/// reason it survived review: an edit-carrying campaign emitted its lethal tick
/// driver, its kill function and its ledger, and the ledger said `"cells": 0`.
/// Every gate over it was green about a world with no kill boxes in it.
///
/// A binding count is a claim that a proof examined something; this one is
/// asserted non-zero on the arm where it was zero.
#[test]
fn the_lethal_ledger_binds_on_the_edit_replay_arm() {
    let c = parse_hw_with_edits(&quests_doc(HARMLESS, ""), Some(SIDE_DOOR));
    let out = build(&c);
    let ledger: serde_json::Value =
        serde_json::from_str(&text(&out, "validation/lethal-gate.json")).unwrap();
    assert_eq!(ledger["volumes"]["resolved"], 1, "the volume resolved");
    assert_ne!(
        ledger["cells"], 0,
        "an edited campaign's nav world carries the volume's cells: {ledger}"
    );
}

/// An objective buried in a volume: thirty cells of the room the party walks are
/// inside the killing box's reach, and the room says nothing about any of them.
///
/// The volume is sized to reach the exit and NOTHING the campaign posts a body
/// on. A `[6, 6, 6]` box swallowed the entry spawn as well, and the seat proof —
/// which runs first, because a Keeper standing in a pit is a more actionable
/// message than the route closure it causes — reported that instead. Three true
/// findings now, and the fixture has to state which one it is about: `DW0891`
/// names the cause, `DW0510` the route it closes, `DW0511` the body it swallows.
#[test]
fn an_objective_buried_in_an_unsignalled_volume_is_dw0891() {
    let c = parse_hw(&quests_doc(
        r#"{ "id": "lethal/the-exit",
             "region": { "anchor": "anchor/exit", "extent": [2, 2, 2] },
             "message": "Nothing here is survivable." }"#,
        "",
    ));
    assert_eq!(failure_code(&c), "DW0891");
}

// --- DW0511: the death loop routing cannot see -----------------------------

/// A respawn seat inside a volume routes perfectly and kills the party on arrival,
/// forever. The seat is reached by teleport, so no reachability proof can see it.
#[test]
fn a_checkpoint_inside_a_volume_is_dw0511() {
    let c = parse_hw(&quests_doc(
        r#"{ "id": "lethal/on-the-seat",
             "region": { "anchor": "anchor/keeper-stand", "extent": [0, 0, 0] },
             "message": "You wake up dying." }"#,
        r#", { "type": "set-checkpoint", "anchor": "anchor/keeper-stand" }"#,
    ));
    assert_eq!(failure_code(&c), "DW0511");
}

/// The same code, the second family it covers: an NPC posted inside a volume is
/// deleted on the first tick — the volume's entity sweep exempts the engine's own
/// machinery and deliberately not content bodies — and no route proof can see it,
/// because a post is a declaration and not a walk. Found while writing this
/// feature's own CI fixture.
#[test]
fn an_npc_posted_inside_a_volume_is_dw0511() {
    let c = parse_hw(&quests_doc(
        r#"{ "id": "lethal/on-the-keeper",
             "region": { "anchor": "anchor/keeper-stand", "extent": [0, 0, 0] },
             "message": "The floor was never there." }"#,
        "",
    ));
    assert_eq!(failure_code(&c), "DW0511");
}

// --- the runtime half + the binding ledger ---------------------------------

/// One PackTest template per volume, and it really puts an entity in the box: the
/// template asserts the dummy is inside the volume's own selector before it asserts
/// the volume killed it, so a template that bound to nothing cannot pass.
#[test]
fn each_volume_gets_a_packtest_that_binds() {
    let c = parse_hw_with_edits(&quests_doc(HARMLESS, ""), Some(SIDE_DOOR));
    let out = build(&c);
    let t = text(
        &out,
        "packtest-datapack/data/hello-world/test/lethal_the_burn.mcfunction",
    );
    assert!(
        t.contains("summon minecraft:zombie"),
        "the template puts a real entity in the volume: {t}"
    );
    assert!(
        t.contains("assert score #in_leth dw.sys matches 1"),
        "the template proves its dummy is INSIDE the volume before asserting the kill: {t}"
    );
    assert!(
        t.contains("function hello-world:lethal_the_burn"),
        "the template drives the volume's real generated function: {t}"
    );
    assert!(
        t.contains("assert score #hp_leth dw.sys matches ..0"),
        "the template asserts the entity died: {t}"
    );

    // The second template, and what it binds to. A PackTest fake player is
    // permanently undamageable (measured: `Health: 20f` unchanged after 202 ticks
    // inside a swinging volume, `minecraft:generic` refused identically), so this
    // tier cannot witness a player DEATH — that belongs to the bot tier. What it
    // can witness is the opposite direction, and the dummy is the ideal fixture
    // for it: a body that provably never dies must never produce the claim.
    let claim = text(
        &out,
        "packtest-datapack/data/hello-world/test/lethal_the_burn_claim.mcfunction",
    );
    assert!(
        claim.contains("scoreboard players set #leth_hp dw.sys 0"),
        "the guard score is baselined to the DEAD sentinel, so a kill function that \
         never ran cannot pass by leaving it untouched: {claim}"
    );
    // The DRIVER, not the kill function: only the driver carries the `@a[<box>]`
    // re-bind, so only driving it binds the template to the player path existing.
    // Measured: with the player line deleted from the driver, a template that
    // called `lethal_<id>_kill` directly still passed 12/12.
    assert!(
        claim.contains("function hello-world:lethal_the_burn\n")
            && !claim.contains("run function hello-world:lethal_the_burn_kill")
            && claim.contains("assert score #leth_hp dw.sys matches 1.."),
        "the template drives the volume's DRIVER (which carries the player re-bind), \
         not its kill function: {claim}"
    );
}

/// The binding ledger states what the proofs looked at. A green over zero volumes
/// or zero legs is a vacuous pass, and this is what makes that legible without
/// re-deriving it from an empty diagnostics list.
#[test]
fn the_binding_ledger_states_its_counts() {
    let c = parse_hw_with_edits(&quests_doc(HARMLESS, ""), Some(SIDE_DOOR));
    let out = build(&c);
    let gate: serde_json::Value =
        serde_json::from_str(&text(&out, "validation/lethal-gate.json")).unwrap();
    assert_eq!(gate["volumes"]["declared"], 1);
    assert_eq!(gate["volumes"]["resolved"], 1);
    assert_eq!(gate["packtest_templates"], 1);
    assert_eq!(gate["unbound"], false);
    assert!(
        gate["cells"].as_u64().unwrap() >= 1,
        "the volume closes at least one world cell: {gate}"
    );
    assert!(
        gate["respawn_seats_examined"].as_u64().unwrap() >= 1,
        "the entry spawn is always a seat: {gate}"
    );
    assert!(
        gate["critical_path_legs_examined"].as_u64().unwrap() >= 1,
        "the route proof examined at least one leg: {gate}"
    );
}

/// Determinism (ADR-0006): two builds of a lethal-volume campaign are byte-equal.
#[test]
fn a_lethal_volume_build_is_byte_identical_across_runs() {
    let c = parse_hw_with_edits(&quests_doc(HARMLESS, ""), Some(SIDE_DOOR));
    assert_eq!(build(&c), build(&c));
}

// --- the CI fixture -------------------------------------------------------

/// The `lethal-volume` fixture — the campaign the tier-2 PackTest pass boots —
/// validates clean and emits its template. Guarded here so a broken fixture
/// reddens in tier 1 (seconds) rather than in a toolserver boot (minutes), and so
/// the CI step can never quietly stop having a volume to prove.
#[test]
fn the_ci_fixture_validates_and_emits_its_template() {
    use delvec::compiler::load::load_campaign_dir;
    use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry};

    let dir = common::compiler_fixtures_dir().join("lethal-volume");
    let loaded = load_campaign_dir(&dir).unwrap();
    let mut c = parse_campaign(&loaded.raw).expect("the lethal-volume fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = delvewright_dsl::validate_campaign_with(
        &c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(diags.is_empty(), "the fixture validates clean: {diags:#?}");

    delvewright_dsl::tag_translatables(&mut c);
    let out = build(&c);
    assert!(
        out.contains_key("packtest-datapack/data/lethal-volume/test/lethal_the_burn.mcfunction"),
        "the fixture emits the template the tier-2 pass runs"
    );
}

// --- DW0891: the population is the lethality-free one ----------------------

/// The `lethal-volume` fixture, planned, with the world and the block map the
/// build judges it over — everything `check_danger_is_visible` takes.
fn with_fixture<R>(
    f: impl FnOnce(&Plan, &delvec::compiler::nav::World, &BTreeMap<[i32; 3], String>) -> R,
) -> R {
    let dir = common::repo_root().join("crates/delvec/tests/fixtures/lethal-volume");
    let raw = delvewright_dsl::RawCampaign {
        world: std::fs::read_to_string(dir.join("world.json")).unwrap(),
        npcs: std::fs::read_to_string(dir.join("npcs.json")).unwrap(),
        classes: std::fs::read_to_string(dir.join("classes.json")).unwrap(),
        quest_plan: std::fs::read_to_string(dir.join("quest-plan.json")).unwrap(),
        quests: std::fs::read_to_string(dir.join("quests.json")).unwrap(),
        dialogue: std::fs::read_to_string(dir.join("dialogue.json")).unwrap(),
        world_edits: Some(std::fs::read_to_string(dir.join("world-edits.json")).unwrap()),
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    let mut c = parse_campaign(&raw).expect("the fixture parses");
    delvewright_dsl::tag_translatables(&mut c);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let s = structures(&plan);
    // The EDITED world — the fixture's magma floor is laid by a batch, and a
    // world built from the placed bytes alone would have stone where the signal
    // is. `assembled::edit_replay` is what `emit::build` takes for a campaign
    // that declares `world-edits`.
    let replay = delvec::compiler::edit::replay(&plan, &prefabs, &s)
        .expect("the edit script replays")
        .expect("the fixture declares world-edits");
    let occ = delvec::compiler::assembled::occupancy_of(
        replay.assembled.blocks.clone(),
        &replay.assembled.open_gates,
    );
    let world = delvec::compiler::nav::World::from_occupancy(
        occ,
        delvec::compiler::nav::Premises::of_plan(&plan, replay.assembled.gate_seals.clone()),
    );
    let blocks = replay.assembled.blocks.clone();
    f(&plan, &world, &blocks)
}

/// **The check reads the LETHALITY-FREE world, and that is its whole binding**
/// (spec-0062 §10.4).
///
/// This is the vacuity trap the spec names, and it is not hypothetical. The walk
/// model already refuses every cell of the keep-out
/// (`World::standable_fp` through `World::meets_lethal_fp`), so a population
/// taken from the world the router walks can never contain a caught cell: the
/// check would be green over every volume ever written while matching nothing at
/// all, and no fixture would ever go red to say so.
///
/// So the perturbation is the vacuous shape itself. The same judgement is taken
/// twice over one fixture, once against each world, and the two numbers are the
/// finding: **zero** caught cells over the lethal-applied world, **nine** over
/// the counterfactual `World::without_lethal` the check really reads. Nine is
/// the floor course the fixture lays in magma, cell for cell.
#[test]
fn the_population_is_the_lethality_free_one() {
    with_fixture(|plan, world, blocks| {
        let entry = plan.campaign_start().map(|(_, pos)| pos);
        assert!(entry.is_some(), "the fixture resolves an entry spawn");

        // The check as it is bound: over the counterfactual, which is what
        // `check_danger_is_visible` builds for itself.
        let (bound, verdict) =
            delvec::compiler::lethal::check_danger_is_visible(plan, world, blocks, entry);
        assert!(verdict.is_ok(), "the fixture is green: {:?}", verdict.err());
        assert_eq!(
            bound.caught(),
            9,
            "over the lethality-free world the volume catches the nine cells of floor \
         the fixture lays in magma: {bound:?}"
        );
        assert_eq!(bound.shown(), 9, "and every one of them shows: {bound:?}");

        // The same judgement over a world whose lethality has ALREADY been applied —
        // the vacuous shape. `without_lethal` is idempotent, so handing the check a
        // world that already carries no volumes is not the perturbation; what is, is
        // taking the population from the world the ROUTER walks. Done here directly,
        // so the number this check would report if it read the wrong world is on the
        // record beside the number it does report.
        let applied = world.reachable_walkable(&[entry.unwrap()]);
        let body = delvewright_dsl::metrics::Body::PLAYER;
        let v = &plan.lethal_volumes[0];
        let (klo, khi) = delvewright_dsl::metrics::keep_out_box(body, v.region.0, v.region.1);
        let caught_over_the_applied_world = applied
            .iter()
            .filter(|c| (0..3).all(|i| klo[i] <= c[i] && c[i] <= khi[i]))
            .count();
        assert_eq!(
            caught_over_the_applied_world, 0,
            "the vacuous shape: over the world the router walks, the keep-out is already \
         impassable, so this population holds no caught cell and the check would be \
         green over every volume ever written"
        );
        assert!(
            bound.population > applied.len(),
            "and the counterfactual population is the larger of the two — {} cell(s) \
         against {}",
            bound.population,
            applied.len()
        );
    });
}

/// **A declared signal on a volume that catches nothing** — `DW0891`'s second
/// shape with an empty caught set, which says a different thing from the same
/// shape with a full one: the volume needs no signal at all, so the declaration
/// is what is wrong.
///
/// The empty population is reached by judging the fixture's own volume over a
/// world with no floor in it at all — a world nothing can stand in, so nothing
/// can be caught. That is the smallest thing that produces the branch, and it is
/// the branch that matters: the message has to say the volume catches nothing
/// rather than report "no cell bears it out" over a set nobody could have filled.
#[test]
fn a_signal_on_a_volume_that_catches_nothing_is_dw0891() {
    with_fixture(|plan, _world, blocks| {
        let empty = delvec::compiler::nav::World::from_solid_and_flooded(
            std::collections::BTreeSet::new(),
            std::collections::BTreeSet::new(),
        );
        let (bound, verdict) =
            delvec::compiler::lethal::check_danger_is_visible(plan, &empty, blocks, None);
        assert_eq!(bound.population, 0, "no floor, no population: {bound:?}");
        assert_eq!(bound.caught(), 0, "and so nothing caught: {bound:?}");
        let f = verdict.expect_err("a declaration nothing bears out is refused");
        assert_eq!(f.code.to_string(), "DW0891");
        assert!(
            f.message.contains("catches no walked floor at all"),
            "the message says the volume needs no signal, not that the list is wrong: {}",
            f.message
        );
        assert!(
            f.message.contains("Delete the declaration"),
            "and it names the move: {}",
            f.message
        );
    });
}

// --- the fixture, perturbed (spec-0062 §8 / criterion 5) -------------------

/// The `lethal-volume` fixture copied into scratch, with one stage document
/// edited, then built. Returns the exit code and everything the run said.
fn fixture_perturbed(
    tag: &str,
    doc: &str,
    f: impl FnOnce(&mut serde_json::Value),
) -> (i32, String) {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lv-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    common::copy_dir_all(
        &common::repo_root().join("crates/delvec/tests/fixtures/lethal-volume"),
        &dir,
    );
    let path = dir.join(doc);
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    f(&mut v);
    std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap() + "\n").unwrap();
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lv-out-{tag}"));
    let _ = std::fs::remove_dir_all(&out);
    let r = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args([
            "build",
            dir.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            common::prefabs_dir().to_str().unwrap(),
        ])
        .output()
        .expect("delvec runs");
    (
        r.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&r.stdout),
            String::from_utf8_lossy(&r.stderr)
        ),
    )
}

/// **The floor is what shows the hazard, and deleting it is `DW0891`.**
///
/// The fixture's `world-edits` lays magma under exactly the nine cells the
/// volume catches. Take that batch out and the volume's declaration is left
/// standing over plain stone: nine cells of walked floor caught, none of them
/// showing anything, and the declared signal borne out by nothing. The refusal
/// names both the floor and the volume — the perturbation is one batch and it
/// reddens loudly rather than quietly.
#[test]
fn the_fixture_without_its_molten_floor_is_dw0891() {
    let (code, log) = fixture_perturbed("no-magma", "world-edits.json", |v| {
        let edits = v["content"]["batches"][0]["edits"].as_array_mut().unwrap();
        // The first two edits ARE the burn: the select and the replace. What is
        // left is the side door alone.
        edits.drain(0..2);
    });
    assert_eq!(code, 3, "an unsignalled floor is refused:\n{log}");
    assert!(log.contains("DW0891"), "{log}");
    assert!(
        log.contains("y=65 (9 cell(s)"),
        "and it names the nine cells of floor the volume catches, by course:\n{log}"
    );
}

/// **At `radius: 1` the completion volume holds no footing, and the move the
/// message names is the radius the fixture ships with** (spec-0062 §7.2).
///
/// A flush hazard's anchor is a cell no body may stand on, so the reach's
/// footing lies two cells out on three sides and the `radius: 1` cube is the
/// keep-out plus a course of air. The number `DW0850` names is verified before
/// it is printed — the judgement is taken again at 2 and comes back green — and
/// 2 is what the fixture ships.
#[test]
fn the_fixture_at_radius_one_is_dw0850_naming_radius_two() {
    let (code, log) = fixture_perturbed("radius-1", "quests.json", |v| {
        v["content"]["quests"][0]["objectives"][1]["radius"] = serde_json::json!(1);
    });
    assert_eq!(code, 3, "a volume with no footing in it is refused:\n{log}");
    assert!(log.contains("DW0850"), "{log}");
    assert!(
        log.contains("set `radius: 2`"),
        "and it names the radius the fixture ships with:\n{log}"
    );
}
