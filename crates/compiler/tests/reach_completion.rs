//! `DW0850`: **a `reach` the party can arrive at without completing.**
//!
//! ## The finding
//!
//! A reach-anchor's completion volume was a point sphere too tight for a human
//! standing on the altar cell, so arriving did not complete the objective. The
//! instance was repaired once, in the emitter, by widening v0.3+ to a 3×3×3
//! cube. Nothing re-asserted it on a build — so the repair covered the volume
//! that had been reported, and the identical defect reached by a *different*
//! number was left live.
//!
//! ## The different number, and it is live on this engine
//!
//! `nav::SNAP_RADIUS` is **3**. The completion cube's half-extent is **1**. The
//! route proof snaps a leg's endpoint to the nearest standable cell within three
//! blocks of the anchor, proves the walk, and exports the waypoint. So a reach
//! whose only footing is two or three blocks out is routable, exportable and
//! walkable — and the party, standing exactly where the campaign routed them,
//! is outside the volume that fires the objective. `DW0311` is green, `DW0314`
//! is green, and the delve stops.
//!
//! ## What is asserted
//!
//! Both halves of the rule, each driven red and then green on the same campaign
//! by moving one thing:
//!
//! * **occupiable** — no cell of the completion volume is standable;
//! * **delivered into** — the arrival the walk proves is outside the volume,
//!   at exactly the snap distance the existing proofs permit.
//!
//! And the v0.2 sphere arm, which is the reported instance itself: the same
//! world and the same anchor complete or do not complete purely by the declared
//! `radius`.
//!
//! The red is confirmed **not inert**: `DW0850` is `every_version`, and
//! `dw0850_binds_at_every_declared_version` drives the identical red at the
//! bottom and the top of the supported range. A rule keyed off a surface the
//! fixture's version never reached would pass by accident; this one cannot.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use delvewright_compiler::nav::World;
use delvewright_compiler::plan::{Plan, Step};
use delvewright_compiler::reach::{
    DW_REACH_UNCOMPLETABLE, ReachVolume, judge_reach_completion, sites,
};
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// hello-world with its exit beat turned into a `reach` on `anchor/exit`.
fn quests(version: &str, radius: u32) -> String {
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
          {{ "type": "reach-anchor", "id": "obj/arrive", "anchor": "anchor/exit",
             "radius": {radius}, "after": ["obj/talk"] }}
        ],
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn campaign(version: &str, radius: u32) -> Campaign {
    parse_campaign(&RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests(version, radius),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
    })
    .expect("campaign parses")
}

/// A `Plan` borrows its campaign, so the campaign has to outlive it — hence a
/// closure rather than a returned `Plan`.
fn with_plan<R>(version: &str, radius: u32, f: impl FnOnce(&Plan) -> R) -> R {
    let c = campaign(version, radius);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    f(&plan)
}

/// The one reach site this fixture declares, and where the plan put it.
fn only_site(plan: &Plan) -> ([i32; 3], String) {
    let s = sites(plan);
    assert_eq!(
        s.len(),
        1,
        "the fixture declares exactly one reach — any other binding means the site \
         enumeration changed and every assertion below is about something else"
    );
    (s[0].pos, s[0].objective_id.clone())
}

/// A world whose only floor is the single solid cell `under`, so exactly the cell
/// above it is standable and nothing else is.
fn floor_at(under: [i32; 3]) -> World {
    let mut solid = BTreeSet::new();
    solid.insert(under);
    World::from_solid_and_flooded(solid, BTreeSet::new())
}

/// The critical-path step index of `objective_id`, so an arrival can be aimed at
/// the leg that serves it.
fn reach_step(plan: &Plan, objective_id: &str) -> usize {
    plan.critical_path
        .iter()
        .position(|s| matches!(s, Step::Reach { objective_id: o, .. } if o == objective_id))
        .expect("the reach objective is a critical-path step")
}

// -------------------------------------------------------------- occupiable --

/// **Green.** The anchor cell itself has floor under it, so a body can stand in
/// the completion volume and arriving completes.
#[test]
fn a_volume_with_footing_in_it_is_clean() {
    with_plan("0.6.0", 2, |plan| {
        let (pos, _) = only_site(plan);
        let world = floor_at([pos[0], pos[1] - 1, pos[2]]);
        assert!(
            world.is_standable(pos),
            "the fixture's premise: the anchor cell is standable"
        );
        judge_reach_completion(plan, &world, &BTreeMap::new())
            .expect("a reach whose own cell is standable completes");
    });
}

/// **Red.** The same anchor with its floor moved four blocks away: no cell of the
/// 3×3×3 cube is standable, so nothing a player can do completes the objective.
/// The finding as a property — a completion volume no body can occupy.
#[test]
fn a_volume_no_body_can_stand_in_is_refused() {
    with_plan("0.6.0", 2, |plan| {
        let (pos, obj) = only_site(plan);
        let world = floor_at([pos[0] + 4, pos[1] - 1, pos[2]]);
        let err = judge_reach_completion(plan, &world, &BTreeMap::new())
            .expect_err("a volume with no standable cell must be refused");
        assert_eq!(err.code, DW_REACH_UNCOMPLETABLE);
        assert!(
            err.message.contains(&obj) && err.message.contains("anchor/exit"),
            "DW0850 names the objective and its anchor: {}",
            err.message
        );
        assert!(
            err.message.contains("no cell of that volume is standable"),
            "the occupiable half reads as itself: {}",
            err.message
        );
    });
}

// ---------------------------------------------------------- delivered into --

/// **Red, and this is the half no existing proof could see.** The volume IS
/// occupiable and the route IS provable — and the endpoint the walk delivers is
/// two blocks out, inside `SNAP_RADIUS` and outside the cube. Green everywhere
/// else; the objective never fires.
#[test]
fn an_arrival_inside_the_snap_radius_but_outside_the_volume_is_refused() {
    with_plan("0.6.0", 2, |plan| {
        let (pos, obj) = only_site(plan);
        let mut solid = BTreeSet::new();
        solid.insert([pos[0], pos[1] - 1, pos[2]]); // the volume is occupiable…
        solid.insert([pos[0] + 2, pos[1] - 1, pos[2]]); // …and the walk ends here
        let world = World::from_solid_and_flooded(solid, BTreeSet::new());
        let arrival = [pos[0] + 2, pos[1], pos[2]];
        assert!(
            world.is_standable(arrival),
            "the fixture's premise: the arrival is a cell the route could snap to"
        );
        let step = reach_step(plan, &obj);

        let arrivals: BTreeMap<usize, [i32; 3]> = [(step, arrival)].into_iter().collect();
        let err = judge_reach_completion(plan, &world, &arrivals)
            .expect_err("an arrival outside the completion volume must be refused");
        assert_eq!(err.code, DW_REACH_UNCOMPLETABLE);
        assert!(
            err.message.contains("outside it"),
            "the delivery half reads as itself: {}",
            err.message
        );

        // …and the SAME world with the walk ending on the anchor's own cell is
        // clean. One thing moved, and it is the one the rule is about.
        let good: BTreeMap<usize, [i32; 3]> = [(step, pos)].into_iter().collect();
        judge_reach_completion(plan, &world, &good)
            .expect("an arrival inside the volume completes");
    });
}

// ------------------------------------------------------- the v0.2 instance --

/// The reported instance itself, re-enacted. Under v0.2 the volume is a
/// `distance=..radius` sphere about the anchor POINT, and a body standing on the
/// cell is measured from its own feet — so `radius: 0` is the point sphere a
/// human standing on the altar cell could not satisfy, and widening the radius
/// is what closed it.
#[test]
fn the_v02_point_sphere_is_the_reported_instance() {
    with_plan("0.2.0", 0, |plan| {
        let (pos, _) = only_site(plan);
        let world = floor_at([pos[0], pos[1] - 1, pos[2]]);
        assert!(world.is_standable(pos));
        let err = judge_reach_completion(plan, &world, &BTreeMap::new())
            .expect_err("a point sphere no standing body satisfies must be refused");
        assert_eq!(err.code, DW_REACH_UNCOMPLETABLE);
        assert!(
            err.message.contains("sphere of radius 0"),
            "the message names the volume that is wrong: {}",
            err.message
        );
    });

    // The same anchor and the same world, with a radius a standing body fits in.
    with_plan("0.2.0", 2, |plan| {
        let (pos, _) = only_site(plan);
        let world = floor_at([pos[0], pos[1] - 1, pos[2]]);
        judge_reach_completion(plan, &world, &BTreeMap::new())
            .expect("a sphere a standing body fits in completes");
    });
}

// --------------------------------------------------------------- the volume --

/// The volume rule is one function, and the emitter formats its selector from
/// this value. Pinned here so a change to either side has to move this number.
#[test]
fn the_cube_is_the_anchor_cell_with_one_block_of_generosity() {
    let v = ReachVolume::of(true, [10, 64, -3], 7);
    assert_eq!(
        v,
        ReachVolume::Cube {
            min: [9, 63, -4],
            max: [11, 65, -2]
        },
        "the v0.3+ volume ignores `radius` entirely — itself worth pinning, because \
         the field is still authored and still reaches the harness"
    );
    assert_eq!(v.cells().len(), 27);
    assert!(v.certainly_completes_from([11, 65, -2]));
    assert!(!v.certainly_completes_from([12, 65, -2]));
}

// ------------------------------------------------------ not-inert guarantee --

/// **The red demo is not inert.** `DW0850` judges the geometry of the world a
/// build assembles, not a surface a campaign opts into, so it is `every_version`
/// and no per-stage fence can grandfather it away. The identical red is driven at
/// the bottom and the top of the supported range and required at both.
#[test]
fn dw0850_binds_at_every_declared_version() {
    for version in ["0.3.0", "0.14.0"] {
        with_plan(version, 2, |plan| {
            let (pos, _) = only_site(plan);
            let world = floor_at([pos[0] + 4, pos[1] - 1, pos[2]]);
            let err = judge_reach_completion(plan, &world, &BTreeMap::new()).expect_err(
                "DW0850 must bind at every declared version; a version-shaped hole here \
                 is the `unfenced` vacuity mode",
            );
            assert_eq!(
                err.code, DW_REACH_UNCOMPLETABLE,
                "at quests {version}: {}",
                err.message
            );
        });
    }
}
