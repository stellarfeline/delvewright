//! A scripted walk is planned from the branch it runs on, and that branch is
//! the conjunction of every gate between it and the bundle's owner — its own
//! `when`, any enclosing effect's `when`, and the gate of the objective (or
//! trigger, trap, loop) whose firing runs the bundle.
//!
//! The defect pinned here: the planner once read a walk's branch off the walk
//! effect's own `when` alone. Two objectives, each gated on its own branch
//! flag, each walked one body somewhere; neither walk carried a `when`. The
//! second walk then chained from the first walk's destination — a staging its
//! own branch can never have produced — so the body snapped across the field
//! before it walked, and no diagnostic fired.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::nav;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, DSL_VERSION, RawCampaign, parse_campaign};

/// Two branch objectives after `obj/talk`, gated on `flag/ground` and
/// `flag/tide`; each walks `actor/ram` from its spawn at `anchor/keeper-stand`.
/// `walk_a` / `walk_b` are the walk effects' extra fields (a `when`, or empty).
/// `wrap` wraps both walks in a one-step `sequence` when true.
fn quests_doc(walk_a: &str, walk_b: &str, wrap: bool) -> String {
    let a = format!(
        r#"{{ "type": "move-actor", "actor": "actor/ram", "to": {{ "anchor": "anchor/exit" }}{walk_a} }}"#
    );
    let b = format!(
        r#"{{ "type": "move-actor", "actor": "actor/ram", "to": {{ "anchor": "anchor/door" }}{walk_b} }}"#
    );
    let (a, b) = if wrap {
        (
            format!(
                r#"{{ "type": "sequence", "steps": [ {{ "at_ticks": 0, "effects": [ {a} ] }} ] }}"#
            ),
            format!(
                r#"{{ "type": "sequence", "steps": [ {{ "at_ticks": 0, "effects": [ {b} ] }} ] }}"#
            ),
        )
    } else {
        (a, b)
    };
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "actors": [
      {{ "id": "actor/ram", "entity": "minecraft:sheep", "anchor": "anchor/keeper-stand" }}
    ],
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/ground", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"], "requires_flags": ["flag/ground"] }},
          {{ "type": "reach-anchor", "id": "obj/tide", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"], "requires_flags": ["flag/tide"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [
            {{ "type": "open-gate", "anchor": "anchor/door" }},
            {{ "type": "set-flag", "flag": "flag/ground" }},
            {{ "type": "set-flag", "flag": "flag/tide" }}
          ],
          "obj/ground": [ {a} ],
          "obj/tide": [ {b} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn parse_hw(quests: &str) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: common::declared_quests_doc(quests, &common::hello_world_dir()),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn structures(plan: &Plan) -> BTreeMap<String, Vec<u8>> {
    let mut out: BTreeMap<String, Vec<u8>> = BTreeMap::new();
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

/// `(destination anchor, first cell, gate key)` of every planned walk.
fn planned(quests: &str) -> Vec<(String, [i32; 3], String)> {
    let c = parse_hw(quests);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let world = nav::World::from_plan(&plan, &structures(&plan));
    nav::plan_actor_moves(&plan, &world)
        .expect("both walks plan")
        .into_iter()
        .map(|m| (m.to.anchor.as_str().to_string(), m.cells[0], m.gate_key))
        .collect()
}

fn build(quests: &str) -> BuildOutput {
    let c = parse_hw(quests);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures(&plan),
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("the campaign builds")
}

fn start_of(walks: &[(String, [i32; 3], String)], anchor: &str) -> [i32; 3] {
    walks
        .iter()
        .find(|(a, _, _)| a == anchor)
        .unwrap_or_else(|| panic!("no planned walk to `{anchor}`: {walks:?}"))
        .1
}

/// **The regression.** Neither walk carries a `when`; each runs from an
/// objective gated on its own branch flag. Both bodies start at the spawn,
/// because neither branch can have run the other's walk.
#[test]
fn a_walk_inherits_the_gate_of_the_objective_that_runs_it() {
    let walks = planned(&quests_doc("", "", false));
    assert_eq!(walks.len(), 2, "two walks planned: {walks:?}");
    let ground = start_of(&walks, "anchor/exit");
    let tide = start_of(&walks, "anchor/door");
    assert_eq!(
        tide, ground,
        "the tide-branch walk must start where the ground-branch walk did (the \
         spawn), never at the ground branch's destination: {walks:?}"
    );
}

/// The same pair one level down, inside a `sequence` step — the shape the demo
/// level that found the defect uses. The objective's gate reaches nested effects.
#[test]
fn a_nested_walk_inherits_the_objective_gate_too() {
    let walks = planned(&quests_doc("", "", true));
    assert_eq!(
        start_of(&walks, "anchor/door"),
        start_of(&walks, "anchor/exit"),
        "{walks:?}"
    );
}

/// The workaround the demo level carried — a `when` restating its objective's
/// gate on each walk — plans exactly as the bare walks do now: the restated
/// term is already in the branch, so origin and driver key agree.
#[test]
fn restating_the_objective_gate_in_when_changes_nothing() {
    let bare = planned(&quests_doc("", "", true));
    let restated = planned(&quests_doc(
        r#", "when": { "requires_flags": ["flag/ground"] }"#,
        r#", "when": { "requires_flags": ["flag/tide"] }"#,
        true,
    ));
    assert_eq!(bare, restated);
}

/// Every emitted call to a walk driver names a driver that was emitted: the
/// emitter's branch key and the planner's are one reading.
#[test]
fn every_walk_call_names_an_emitted_driver() {
    let out = build(&quests_doc("", "", true));
    let fns: Vec<String> = out
        .keys()
        .filter_map(|p| p.strip_prefix("datapack/data/hello-world/function/"))
        .filter_map(|n| n.strip_suffix(".mcfunction"))
        .map(str::to_string)
        .collect();
    let mut calls = 0usize;
    for body in out.values() {
        let Ok(text) = std::str::from_utf8(body) else {
            continue;
        };
        for word in text.split_whitespace() {
            if let Some(name) = word.strip_prefix("hello-world:")
                && name.starts_with("ma_")
            {
                calls += 1;
                assert!(
                    fns.iter().any(|f| f == name),
                    "`{name}` is called but not emitted"
                );
            }
        }
    }
    assert!(calls >= 2, "both walk calls were found ({calls})");
}

/// The `quests_doc` shape with the actor walks swapped for `move-npc`s of
/// `npc/keeper` (home `anchor/keeper-stand`): the NPC planner reads the same
/// branch.
fn npc_doc() -> String {
    quests_doc("", "", false)
        .replace(
            r#""type": "move-actor", "actor": "actor/ram""#,
            r#""type": "move-npc", "npc": "npc/keeper""#,
        )
        .replace(
            r#"{ "id": "actor/ram", "entity": "minecraft:sheep", "anchor": "anchor/keeper-stand" }"#,
            "",
        )
        .replace(r#""actors": [
      
    ],"#, "")
}

/// `move-npc` shares the planner shape, so it shares the fix.
#[test]
fn a_move_npc_inherits_the_gate_of_the_objective_that_runs_it() {
    let doc = npc_doc();
    assert!(!doc.contains("move-actor"), "fixture swap applied: {doc}");
    let c = parse_hw(&doc);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let world = nav::World::from_plan(&plan, &structures(&plan));
    let moves = nav::plan_moves(&plan, &world).expect("both walks plan");
    assert_eq!(moves.len(), 2, "two walks planned");
    let start = |a: &str| {
        moves
            .iter()
            .find(|m| m.to.anchor.as_str() == a)
            .unwrap_or_else(|| panic!("no walk to `{a}`"))
            .cells[0]
    };
    assert_eq!(start("anchor/door"), start("anchor/exit"));
}

/// Two branches walk the body to ONE mark with no `when` on either: two
/// drivers, one per branch, and a cutscene in each branch follows its own.
#[test]
fn one_mark_two_objective_branches_runs_two_drivers_and_the_shot_follows_its_own() {
    let shot = r#"{ "type": "cutscene", "shots": [ { "shot_style": "locked-off", "seconds": 3, "dist": 6, "subject": { "actor": "actor/ram" } } ] }"#;
    let doc = quests_doc("", "", false)
        .replace(
            r#""to": { "anchor": "anchor/door" }"#,
            r#""to": { "anchor": "anchor/exit" }"#,
        )
        .replace(
            r#""obj/ground": [ "#,
            &format!(r#""obj/ground": [ {shot}, "#),
        )
        // A second shot that differs, so each branch's cutscene is its own
        // function rather than one content-keyed cutscene shared by both.
        .replace(
            r#""obj/tide": [ "#,
            &format!(
                r#""obj/tide": [ {}, "#,
                shot.replace(r#""dist": 6"#, r#""dist": 7"#)
            ),
        );
    let walks = planned(&doc);
    assert_eq!(walks.len(), 2, "one driver per branch: {walks:?}");
    assert_ne!(walks[0].2, walks[1].2, "the drivers' branch keys differ");
    assert!(walks.iter().all(|w| !w.2.is_empty()), "{walks:?}");

    let c = parse_hw(&doc);
    let units = delvec::compiler::camera::cutscene_units(&c);
    let mut keys: Vec<String> = units
        .iter()
        .flat_map(|(_, ctx)| ctx.iter().map(|m| m.gate_key.clone()))
        .collect();
    keys.sort();
    let mut planned_keys: Vec<String> = walks.iter().map(|w| w.2.clone()).collect();
    planned_keys.sort();
    assert_eq!(
        keys, planned_keys,
        "each cutscene's sibling move carries its own driver's branch key"
    );
    let out = build(&doc);
    let drivers = out
        .keys()
        .filter(|p| p.contains("/function/ma_tick_ram_exit_b"))
        .count();
    assert_eq!(drivers, 2, "two emitted drivers to `anchor/exit`");
}

/// A shot follows the driver its own branch runs. The ground branch walks the
/// body to the door and then on to the exit; the tide branch walks it straight
/// to the exit from its spawn. Two drivers reach `anchor/exit` from two
/// places, and each branch's cutscene must aim along its own.
#[test]
fn a_shot_follows_its_own_branchs_driver_to_a_shared_mark() {
    let shot = |dist: u32| {
        format!(
            r#"{{ "type": "cutscene", "shots": [ {{ "shot_style": "locked-off", "seconds": 3, "dist": {dist}, "subject": {{ "actor": "actor/ram" }} }} ] }}"#
        )
    };
    let walk = |to: &str| {
        format!(r#"{{ "type": "move-actor", "actor": "actor/ram", "to": {{ "anchor": "{to}" }} }}"#)
    };
    let ground = format!(
        r#"{{ "type": "sequence", "steps": [
            {{ "at_ticks": 0, "effects": [ {} ] }},
            {{ "at_ticks": 100, "effects": [ {}, {} ] }} ] }}"#,
        walk("anchor/door"),
        walk("anchor/exit"),
        shot(6)
    );
    let tide = format!("{}, {}", walk("anchor/exit"), shot(7));
    let base = quests_doc("", "", false);
    let a = r#"{ "type": "move-actor", "actor": "actor/ram", "to": { "anchor": "anchor/exit" } }"#;
    let b = r#"{ "type": "move-actor", "actor": "actor/ram", "to": { "anchor": "anchor/door" } }"#;
    assert_eq!(base.matches(a).count(), 1);
    assert_eq!(base.matches(b).count(), 1);
    let doc = base.replace(a, &ground).replace(b, &tide);

    let c = parse_hw(&doc);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let world = nav::World::from_plan(&plan, &structures(&plan));
    let actor_moves = nav::plan_actor_moves(&plan, &world).expect("walks plan");
    let to_exit: Vec<_> = actor_moves
        .iter()
        .filter(|m| m.to.anchor.as_str() == "anchor/exit")
        .collect();
    assert_eq!(to_exit.len(), 2, "one driver per branch to the exit");
    assert_ne!(
        to_exit[0].waypoints[0], to_exit[1].waypoints[0],
        "the two drivers start in different places"
    );

    let units = delvec::compiler::camera::cutscene_units(&c);
    assert_eq!(units.len(), 2, "one cutscene per branch");
    let mut aimed = Vec::new();
    for (eff, ctx) in &units {
        let shots = eff.cutscene_shots().expect("a cutscene");
        let x = delvec::compiler::camera::expand_shot(&plan, &[], &actor_moves, &shots[0], ctx, 0);
        let delvec::compiler::camera::AimTrack::Moving(track) = x.aim else {
            panic!("the shot follows the walking body: {:?}", x.aim);
        };
        let key = &ctx
            .iter()
            .find(|m| m.to.anchor.as_str() == "anchor/exit")
            .expect("the exit walk is a sibling move")
            .gate_key;
        let own = to_exit
            .iter()
            .find(|m| &m.gate_key == key)
            .expect("the sibling's key names a planned driver");
        let w = own.waypoints[0];
        assert_eq!(
            track[0],
            [w[0], w[1] + 1.0, w[2]],
            "the shot aims along its own branch's driver"
        );
        aimed.push(track[0]);
    }
    assert_ne!(aimed[0], aimed[1], "the two shots follow different walks");
}
