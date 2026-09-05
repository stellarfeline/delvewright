//! **One authority on where a `reach-anchor` completes**, and the proof that a
//! body can actually be there — asserted on a built campaign rather than
//! remembered.
//!
//! Two defects meet in this file. They were found independently, they have the
//! same shape, and the merge that brought them together is what makes the union
//! below necessary: *a fact with one authority that had quietly grown a second
//! one, and nothing anywhere compared them.*
//!
//! ## The volume against the walk goal
//!
//! `radius` is authored once and had two readers that stopped agreeing. The M2
//! repair for a completion sphere too tight to stand in (`hv-01`) replaced the
//! sphere with a fixed ±1 cube at DSL v0.3 — and *replaced* is the defect: the
//! authored number stopped reaching the datapack entirely, while the harness went
//! on deriving its walk goal from it and aiming `radius - 1` blocks out. For any
//! `radius` of 3 or more that goal is outside the box the server tests, so the
//! bot was entitled to stop short and then hang on a completion that could not
//! fire. It stayed green because a `GoalNear` usually overshoots inward, which
//! made the failure intermittent — and an intermittent failure is an
//! under-specified test, never a flake.
//!
//! ## The volume against the footing (`DW0850`)
//!
//! A reach-anchor's completion volume was a point sphere too tight for a human
//! standing on the altar cell, so arriving did not complete the objective. The
//! instance was repaired once, in the emitter, by widening v0.3+ to a cube.
//! Nothing re-asserted it on a build — so the repair covered the volume that had
//! been reported, and the identical defect reached by a *different* number was
//! left live: `nav::SNAP_RADIUS` is **3**, and a reach whose only footing is
//! further out than the volume reaches is routable, exportable and walkable while
//! the party, standing exactly where the campaign routed them, is outside the
//! volume that fires the objective. `DW0311` is green, `DW0314` is green, and the
//! delve stops.
//!
//! ## What the union asserts
//!
//! 1. **The rule has one home** — [`delvewright_compiler::reach::reach_completion`].
//!    The v0.3+ half-extent is a **floor** over the ±1 that closed `hv-01`
//!    (`max(1, radius)`), not a constant instead of it, and the pre-v0.3 sphere is
//!    untouched.
//! 2. **The judge reads that value** — `DW0850`'s two halves, *occupiable* and
//!    *delivered into*, each driven red and then green on the same campaign by
//!    moving one thing.
//! 3. **The emitted bytes agree with the exported artifact** — for every reach
//!    objective of every campaign built here, the region the tick line adjudicates
//!    with is the region `critical-path.json` hands the bot. Checked on emitted
//!    bytes, not on the fact that both currently call one function, which is a
//!    property of today's source.
//!
//! The version binding is taken from [`envelope::SUPPORTED_DSL_VERSIONS`] — the
//! ledger itself, never a literal — so a new `dsl_version` enters these proofs
//! the moment it lands. A test that has to be edited every time the thing it
//! describes moves was asserting the literal, not the property.
//!
//! Sections 1–2 are unit-level; section 3 drives the real `delvec` binary, like
//! `tests/cli.rs`.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use delvewright_compiler::nav::{SNAP_RADIUS, World};
use delvewright_compiler::plan::{Plan, Step};
use delvewright_compiler::reach::{
    DW_REACH_OFF_FLOOR, DW_REACH_UNCOMPLETABLE, ReachCompletion, check_reach_footprint,
    judge_reach_completion, reach_completion, sites,
};
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::envelope::{self, is_v03};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

// ============================================================ unit fixtures ==

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
        detail_plan: None,
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

// ================================================= 1. the rule has one home ==

/// The volume rule is one function, and every other site formats or judges the
/// value it returns. Pinned here so a change to the rule has to move this number.
///
/// `radius: 7` is the case that used to prove the *opposite*: before the authored
/// number reached the datapack, this call returned the ±1 cube whatever was
/// written. It now returns ±7, and the cell count is the same fact stated a
/// second way.
#[test]
fn the_cube_honours_the_authored_radius() {
    let v = reach_completion([10, 64, -3], 7, true);
    assert_eq!(
        v,
        ReachCompletion::Cube {
            lo: [3, 57, -10],
            hi: [17, 71, 4]
        },
        "the v0.3+ cube's half-extent is the authored `radius`; a fixed extent here \
         is the drift this file exists to refuse"
    );
    assert_eq!(v.cells().len(), 15 * 15 * 15);
    assert!(v.certainly_completes_from([17, 71, 4]));
    assert!(!v.certainly_completes_from([18, 71, 4]));

    // The FLOOR, at the same site: `hv-01` was a volume too tight for a body
    // standing on the anchor cell, and the answer to it must survive every radius
    // an author can write — including the degenerate one.
    assert_eq!(
        reach_completion([10, 64, -3], 0, true),
        ReachCompletion::Cube {
            lo: [9, 63, -4],
            hi: [11, 65, -2]
        },
        "never tighter than the ±1 cube that closed hv-01"
    );
}

/// The floor and the identity, over the whole authorable range, plus the
/// untouched pre-v0.3 arm.
#[test]
fn the_smallest_authorable_volume_is_still_the_cube_that_closed_hv01() {
    for r in 1..=8u32 {
        let ReachCompletion::Cube { lo, hi } = reach_completion([10, 20, 30], r, true) else {
            panic!("v0.3+ is a cube");
        };
        let half = (hi[0] - lo[0]) / 2;
        assert!(half >= 1, "radius {r} must never be tighter than ±1");
        assert_eq!(half, r as i32, "radius {r} means radius {r}");
    }
    assert!(matches!(
        reach_completion([10, 20, 30], 4, false),
        ReachCompletion::Sphere { radius: 4, .. }
    ));
}

/// **The version binding is the ledger's, not a literal's.** Every declared
/// `dsl_version` gets the arm its own fence chooses, and the v0.3+ arm honours the
/// radius at every one of them. A rule keyed off a surface a fixture's version
/// never reached would pass by accident; this cannot, and a new ledger row joins
/// it without anybody editing this file.
#[test]
fn every_declared_version_gets_the_arm_its_fence_chooses() {
    let versions = envelope::SUPPORTED_DSL_VERSIONS;
    assert!(
        versions.len() >= 14,
        "binding: {} declared dsl_version(s) examined",
        versions.len()
    );
    let (pos, radius) = ([10, 64, -3], 4u32);
    for v in versions {
        let vol = reach_completion(pos, radius, is_v03(v));
        if is_v03(v) {
            assert_eq!(
                vol,
                ReachCompletion::Cube {
                    lo: [6, 60, -7],
                    hi: [14, 68, 1]
                },
                "at dsl_version {v} the cube must honour the authored radius"
            );
        } else {
            assert_eq!(
                vol,
                ReachCompletion::Sphere { pos, radius },
                "at dsl_version {v} the pre-v0.3 sphere is emitted byte-identically"
            );
        }
    }
}

// ============================================================ 2. the judge ==

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
/// volume the author declared is standable, so nothing a player can do completes
/// the objective. The finding as a property — a completion volume no body can
/// occupy.
#[test]
fn a_volume_no_body_can_stand_in_is_refused() {
    with_plan("0.6.0", 2, |plan| {
        let (pos, obj) = only_site(plan);
        let footing = [pos[0] + 4, pos[1], pos[2]];
        let world = floor_at([footing[0], footing[1] - 1, footing[2]]);
        // The premise is CHECKED rather than assumed: the volume is the authored
        // radius now, so "four blocks away" is only outside it while the fixture
        // authors a radius below four.
        assert!(
            !reach_completion(pos, 2, true).certainly_completes_from(footing),
            "the fixture's premise: the only footing lies outside the completion volume"
        );
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
        assert!(
            err.message.contains("5×5×5"),
            "the diagnostic describes the volume it is actually refusing, measured off \
             the value — a fixed `3×3×3` here would be a message that lies: {}",
            err.message
        );
    });
}

/// **Red, and this is the half no existing proof could see.** The volume IS
/// occupiable and the route IS provable — and the endpoint the walk delivers is
/// two blocks out, inside `SNAP_RADIUS` and outside the cube. Green everywhere
/// else; the objective never fires.
///
/// The fixture authors `radius: 1`, and that number is load-bearing: the gap this
/// half of `DW0850` lives in is `SNAP_RADIUS` minus the completion half-extent,
/// and the half-extent is now the authored radius. See
/// [`the_delivered_into_half_cannot_fire_at_or_above_the_snap_radius`] for the
/// other end of the same arithmetic.
#[test]
fn an_arrival_inside_the_snap_radius_but_outside_the_volume_is_refused() {
    with_plan("0.6.0", 1, |plan| {
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
        assert!(
            (arrival[0] - pos[0]) <= SNAP_RADIUS
                && !reach_completion(pos, 1, true).certainly_completes_from(arrival),
            "the fixture's premise: the arrival is inside the snap radius and outside \
             the completion volume — the gap the rule is about"
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

/// **The interaction between the two defects, stated rather than discovered.**
///
/// A leg endpoint is snapped inside a box of half-extent [`SNAP_RADIUS`]
/// (`nav::snap_endpoint`), and the v0.3+ completion volume is a box of half-extent
/// `max(1, radius)`. So once an author writes a radius at or above the snap
/// radius, **every arrival the route can possibly deliver is inside the volume**
/// and the *delivered into* half of `DW0850` is structurally unable to fire. That
/// is not a hole: at those radii the datapack tests the same box the walk was
/// snapped into, so there is no defect left to catch. Before the authored radius
/// reached the datapack the two were 3 and 1 and the gap was permanent.
///
/// It is pinned because it is a fact about the *pair*, owned by neither change,
/// and because a later narrowing of the completion volume would silently re-open
/// the gap while every test on either side stayed green.
#[test]
fn the_delivered_into_half_cannot_fire_at_or_above_the_snap_radius() {
    let pos = [10, 64, -3];
    for radius in (SNAP_RADIUS as u32)..=8 {
        let vol = reach_completion(pos, radius, true);
        for dx in -SNAP_RADIUS..=SNAP_RADIUS {
            for dy in -SNAP_RADIUS..=SNAP_RADIUS {
                for dz in -SNAP_RADIUS..=SNAP_RADIUS {
                    let arrival = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
                    assert!(
                        vol.certainly_completes_from(arrival),
                        "radius {radius}: {arrival:?} is snappable and must complete"
                    );
                }
            }
        }
    }
    // …and below the snap radius the gap is real, which is why the fixture above
    // authors 1.
    assert!(
        !reach_completion(pos, 2, true).certainly_completes_from([pos[0] + 3, pos[1], pos[2]]),
        "at radius 2 a snapped arrival can still land outside the volume"
    );
}

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

/// **The red demo is not inert.** `DW0850` judges the geometry of the world a
/// build assembles, not a surface a campaign opts into, so it is `every_version`
/// and no per-stage fence can grandfather it away. The identical red is driven at
/// **every** version the ledger declares — not at two literals that go stale the
/// next time the ledger moves.
#[test]
fn dw0850_binds_at_every_declared_version() {
    let versions = envelope::SUPPORTED_DSL_VERSIONS;
    for version in versions {
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
    assert!(
        versions.len() >= 14,
        "binding: {} declared dsl_version(s) examined",
        versions.len()
    );
}

// ================================================ 2b. the footprint (DW0881) ==

/// A hall floor and a dais standing three courses over it, both wide enough to
/// fill whatever completion volume is authored, with nothing joining the two.
///
/// The anchor's own cell is the dais top, so `DW0850` is satisfied by
/// construction: the volume holds footing. The only question left is whether
/// everything ELSE the volume reaches is that same floor.
fn hall_and_dais(anchor: [i32; 3], half: i32) -> BTreeSet<[i32; 3]> {
    let mut solid = BTreeSet::new();
    for dx in -half..=half {
        for dz in -half..=half {
            // The hall floor: the block three courses under the dais top, so a
            // body down there stands at `anchor[1] - 3`.
            solid.insert([anchor[0] + dx, anchor[1] - 4, anchor[2] + dz]);
        }
    }
    for dx in -1..=1 {
        for dz in -1..=1 {
            solid.insert([anchor[0] + dx, anchor[1] - 1, anchor[2] + dz]);
        }
    }
    solid
}

/// Where the party comes in: a hall-floor cell four out from the dais, outside
/// every volume these fixtures author and connected to the whole hall floor. The
/// population `DW0881` draws its footprint from is walked from here, so handing it
/// explicitly is what keeps these tests about the rule rather than about whatever
/// `hello-world` happens to declare as its start.
fn entry(anchor: [i32; 3]) -> [i32; 3] {
    [anchor[0] + 4, anchor[1] - 3, anchor[2] + 4]
}

/// The same hall and dais with a flight climbing the dais's near face, every
/// tread inside a radius-3 cube: standing cells at `y-2` on `dz = 3` and `y-1` on
/// `dz = 2`, which is three one-block steps from the hall floor to the top.
fn with_a_flight(anchor: [i32; 3], half: i32) -> BTreeSet<[i32; 3]> {
    let mut solid = hall_and_dais(anchor, half);
    for dx in -1..=1 {
        solid.insert([anchor[0] + dx, anchor[1] - 3, anchor[2] + 3]);
        solid.insert([anchor[0] + dx, anchor[1] - 2, anchor[2] + 2]);
    }
    solid
}

/// **Red — the gallery's own former defect, as a property.**
///
/// A `reach` on an anchor three courses over a hall floor, at a radius that
/// reaches that floor, and nothing inside the volume joins the two levels. A
/// party standing in the hall completes an objective whose whole content is
/// *climb*. `DW0850` is green over the same world, which is the point: the volume
/// holds footing, that footing is the dais, and the defect is everything else the
/// volume also holds.
#[test]
fn a_raised_anchor_whose_volume_reaches_the_floor_below_is_refused() {
    with_plan("0.6.0", 3, |plan| {
        let (pos, obj) = only_site(plan);
        let world = World::from_solid_and_flooded(hall_and_dais(pos, 5), BTreeSet::new());
        // The premises, checked rather than assumed.
        assert!(
            world.is_standable(pos),
            "the anchor's own cell is the dais top"
        );
        assert!(
            world.is_standable([pos[0] + 3, pos[1] - 3, pos[2]]),
            "and the hall floor three courses down is standable"
        );
        judge_reach_completion(plan, &world, &BTreeMap::new())
            .expect("DW0850 is green here: the volume holds the dais");
        assert!(
            world.is_standable(entry(pos)),
            "the fixture's premise: the party has somewhere to come in"
        );

        let (binding, verdict) = check_reach_footprint(plan, &world, Some(entry(pos)));
        let err = verdict.expect_err(
            "a completion volume that reaches the floor below its anchor must be refused",
        );
        assert_eq!(err.code, DW_REACH_OFF_FLOOR);
        assert_eq!(binding.sites, 1, "binding: reach objectives examined");
        assert!(
            binding.off_floor > 0 && binding.off_floor < binding.cells,
            "binding: {} of {} footprint cell(s) off the anchor's floor — all or none \
             would mean the walk inside the volume is not being run",
            binding.off_floor,
            binding.cells
        );
        assert!(
            err.message.contains(&obj) && err.message.contains("anchor/exit"),
            "DW0881 names the objective and its anchor: {}",
            err.message
        );
        assert!(
            err.message.contains(&format!("y={}", pos[1] - 3)),
            "and the floor the offending cells stand on: {}",
            err.message
        );
        assert!(
            err.message.contains("7×7×7"),
            "and the volume it is refusing, measured off the value: {}",
            err.message
        );
    });
}

/// **Green, on the same geometry, with the one thing the rule is about moved.**
///
/// A flight of single-block steps *inside the volume* joins the hall to the dais.
/// Every cell the volume reaches can now walk to the anchor's own footing without
/// leaving it, and a body on the flight is arriving rather than standing in
/// another room. A rule that only ever rewarded lowering the radius would refuse
/// this too, and would be pointed one way.
#[test]
fn a_way_up_inside_the_volume_is_arriving_and_passes() {
    with_plan("0.6.0", 3, |plan| {
        let (pos, _) = only_site(plan);
        let world = World::from_solid_and_flooded(with_a_flight(pos, 5), BTreeSet::new());
        // The premise: the treads really are standable, and really are inside the
        // cube the radius authors.
        let vol = reach_completion(pos, 3, true);
        for tread in [
            [pos[0], pos[1] - 2, pos[2] + 3],
            [pos[0], pos[1] - 1, pos[2] + 2],
        ] {
            assert!(
                world.is_standable(tread),
                "{tread:?} is a tread a body stands on"
            );
            assert!(
                vol.certainly_completes_from(tread),
                "{tread:?} is inside the completion volume, which is what makes the \
                 flight a way up INSIDE it"
            );
        }

        let (binding, verdict) = check_reach_footprint(plan, &world, Some(entry(pos)));
        verdict.expect("a volume whose floors are joined inside it completes by arriving");
        assert_eq!(binding.off_floor, 0);
        assert!(
            binding.cells > 0,
            "binding: {} footprint cell(s) — a green over an empty footprint is the \
             unbound vacuity mode, not a pass",
            binding.cells
        );
    });
}

/// **The volume that fits its place is silent**, which is the instance fix: the
/// same hall and the same dais, at a radius whose cube stops above the floor
/// below. One number moved.
#[test]
fn a_volume_that_stops_above_the_lower_floor_is_silent() {
    with_plan("0.6.0", 1, |plan| {
        let (pos, _) = only_site(plan);
        let world = World::from_solid_and_flooded(hall_and_dais(pos, 5), BTreeSet::new());
        let (binding, verdict) = check_reach_footprint(plan, &world, Some(entry(pos)));
        verdict.expect("at radius 1 the cube stops two courses above the hall floor");
        assert_eq!(binding.off_floor, 0);
        assert!(
            binding.cells > 0,
            "binding: {} footprint cell(s)",
            binding.cells
        );
    });
}

/// **The arithmetic the rule turns on, isolated.** Vanilla adjudicates the
/// selector against the whole body box, so a standing cell one course BELOW the
/// volume's bottom layer completes and one two courses below does not. That one
/// course is the entire gallery instance, and a reading that tested the feet cell
/// instead would answer no to both.
#[test]
fn a_body_one_course_under_the_volume_still_reaches_into_it() {
    let vol = reach_completion([0, 68, 0], 2, true);
    let ReachCompletion::Cube { lo, .. } = vol else {
        panic!("v0.3+ is a cube");
    };
    assert_eq!(lo[1], 66);
    assert!(
        vol.possibly_completes_from([0, 65, 0], 65.0),
        "a body standing at 65 rises to 66.8 and is inside a cube whose floor is 66"
    );
    assert!(
        !vol.possibly_completes_from([0, 64, 0], 64.0),
        "a body standing at 64 rises to 65.8 and is not"
    );
    assert!(
        !vol.certainly_completes_from([0, 65, 0]),
        "and the conservative reading DW0850 takes says no to the same cell, which is \
         why the two are named apart"
    );
}

/// **The red demo is not inert.** `DW0881` judges assembled geometry rather than
/// a surface a campaign opts into, so it is `every_version` and no per-stage fence
/// can grandfather it away. Driven at every version the ledger declares, which
/// exercises the pre-v0.3 sphere arm as well as the cube: the radius is 4 because
/// a sphere has to be that wide before it reaches a floor three courses down.
#[test]
fn dw0881_binds_at_every_declared_version() {
    let versions = envelope::SUPPORTED_DSL_VERSIONS;
    let mut bound = 0usize;
    for version in versions {
        with_plan(version, 4, |plan| {
            let (pos, _) = only_site(plan);
            let world = World::from_solid_and_flooded(hall_and_dais(pos, 6), BTreeSet::new());
            let (_, verdict) = check_reach_footprint(plan, &world, Some(entry(pos)));
            let err = verdict.expect_err(
                "DW0881 must bind at every declared version; a version-shaped hole here is \
                 the `unfenced` vacuity mode",
            );
            assert_eq!(err.code, DW_REACH_OFF_FLOOR, "at quests {version}");
            bound += 1;
        });
    }
    assert_eq!(
        bound,
        versions.len(),
        "binding: every declared dsl_version drove the red"
    );
    assert!(
        versions.len() >= 14,
        "binding: {} version(s)",
        versions.len()
    );
}

// =============================================== 3. the agreement, on bytes ==

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// `plan::safe_local`'s shape, as the emitted function name carries it:
/// `obj/reach-the-counter` → `o_reach_the_counter`.
fn obj_fn(obj_id: &str) -> String {
    let local = obj_id.rsplit('/').next().unwrap_or(obj_id);
    format!(
        "o_{}",
        local
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>()
    )
}

/// Build a fixture and return `(critical-path.json, every tick line)`.
fn build(fixture: &str, out_name: &str) -> (serde_json::Value, Vec<String>) {
    let dir = common::compiler_fixtures_dir().join(fixture);
    let out = tmp(out_name);
    let r = Command::new(BIN)
        .args([
            "build",
            dir.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            &common::prefabs_dir().display().to_string(),
        ])
        .current_dir(common::repo_root())
        .output()
        .expect("run delvec");
    assert!(
        r.status.success(),
        "{fixture} builds:\n{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    let path: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("critical-path.json")).expect("path json"))
            .expect("parse path");
    let mut ticks = Vec::new();
    let mut stack = vec![out.join("datapack")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).expect("read dir") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().is_some_and(|n| n == "tick.mcfunction") {
                ticks.extend(
                    std::fs::read_to_string(&p)
                        .expect("read tick")
                        .lines()
                        .map(str::to_string),
                );
            }
        }
    }
    assert!(!ticks.is_empty(), "{fixture} emitted a tick function");
    (path, ticks)
}

/// The exported reach steps of a built campaign, as `(objective, step)`.
fn reach_steps(path: &serde_json::Value) -> Vec<serde_json::Value> {
    path["steps"]
        .as_array()
        .expect("steps array")
        .iter()
        .filter(|s| s["action"] == "reach")
        .cloned()
        .collect()
}

/// The `@s[...]` selector arguments a completion volume denotes — the string the
/// tick line must carry. Written out here from the exported JSON rather than
/// borrowed from the compiler, so this test is a second reader of the artifact
/// and not the emitter agreeing with itself.
fn expected_selector(completion: &serde_json::Value) -> String {
    let v = |k: &str, i: usize| completion[k][i].as_i64().expect("coord");
    match completion["kind"].as_str().expect("kind") {
        "cube" => format!(
            "x={},dx={},y={},dy={},z={},dz={}",
            v("lo", 0),
            v("hi", 0) - v("lo", 0),
            v("lo", 1),
            v("hi", 1) - v("lo", 1),
            v("lo", 2),
            v("hi", 2) - v("lo", 2)
        ),
        "sphere" => format!(
            "x={},y={},z={},distance=..{}",
            v("pos", 0),
            v("pos", 1),
            v("pos", 2),
            completion["radius"].as_u64().expect("radius")
        ),
        other => panic!("unknown completion kind {other}"),
    }
}

/// The agreement, on emitted bytes: for every reach objective, the tick line that
/// completes it adjudicates in exactly the region `critical-path.json` exports.
///
/// The failure this refuses is silent by construction — the datapack keeps
/// compiling, the artifact keeps parsing, and the only symptom is a bot that
/// stops somewhere the objective cannot see it.
fn assert_agreement(fixture: &str, path: &serde_json::Value, ticks: &[String]) -> usize {
    let steps = reach_steps(path);
    for step in &steps {
        let obj = step["objective"].as_str().expect("objective");
        let want = expected_selector(&step["completion"]);
        let needle = format!("complete_{}", obj_fn(obj));
        let lines: Vec<&String> = ticks
            .iter()
            .filter(|l| l.ends_with(&needle) && l.contains("if entity @s["))
            .collect();
        assert_eq!(
            lines.len(),
            1,
            "{fixture}: {obj} should have exactly one adjudicating tick line, got {}",
            lines.len()
        );
        let line = lines[0];
        let start = line.find("if entity @s[").expect("selector") + "if entity @s[".len();
        let end = start + line[start..].find(']').expect("selector close");
        assert_eq!(
            &line[start..end],
            want,
            "{fixture}: {obj} — the datapack adjudicates a different region from the one \
             `critical-path.json` hands the bot. These are two readers of one authored \
             `radius`, and this is exactly the drift that let a bot stop outside the box \
             and hang. Fix the emitter, never this assertion.\n  line: {line}"
        );
    }
    steps.len()
}

/// v0.2 emission is untouched: the pre-v0.3 sphere, both in the artifact and in
/// the line. `hello-world` / `keep-crawl` stay byte-identical.
#[test]
fn a_v02_campaign_keeps_the_pre_v03_sphere() {
    let (path, ticks) = build("v06-edits", "reach-completion-v02");
    let steps = reach_steps(&path);
    assert_eq!(steps.len(), 1, "the fixture has one reach objective");
    assert_eq!(steps[0]["completion"]["kind"], "sphere");
    assert_eq!(steps[0]["completion"]["radius"], steps[0]["radius"]);
    assert_eq!(assert_agreement("v06-edits", &path, &ticks), 1);
}

/// v0.3+ honours the authored radius. `v04-showcase` authors `radius: 2`, so the
/// volume is a cube of half-extent 2 — not the ±1 the emitter used to hard-code,
/// and not tighter than it either.
#[test]
fn a_v04_campaign_gets_the_radius_it_authored() {
    let (path, ticks) = build("v04-showcase", "reach-completion-v04");
    let steps = reach_steps(&path);
    assert_eq!(steps.len(), 1);
    let (c, pos) = (&steps[0]["completion"], &steps[0]["pos"]);
    assert_eq!(c["kind"], "cube");
    assert_eq!(steps[0]["radius"], 2);
    for i in 0..3 {
        let p = pos[i].as_i64().unwrap();
        assert_eq!(c["lo"][i].as_i64().unwrap(), p - 2, "half-extent 2 below");
        assert_eq!(c["hi"][i].as_i64().unwrap(), p + 2, "half-extent 2 above");
    }
    assert_eq!(assert_agreement("v04-showcase", &path, &ticks), 1);
}

/// The same rule on a document the newest pipeline built: a proof that is only
/// shown at the version its surface was introduced at has demonstrated the fence,
/// not the rule. `blockout` authors `radius: 3` — the live shape, and the one the
/// old ±1 box was three times too small for.
///
/// The *version* half of that obligation is discharged by
/// [`every_declared_version_gets_the_arm_its_fence_chooses`] and
/// [`dw0850_binds_at_every_declared_version`], which enumerate the ledger rather
/// than name a version; this one is here for the **emitted bytes**, which need a
/// buildable document and so a real fixture.
#[test]
fn a_derived_world_honours_the_radius_too() {
    let (path, ticks) = build("blockout", "reach-completion-blockout");
    let steps = reach_steps(&path);
    assert!(!steps.is_empty(), "the fixture has reach objectives");
    for step in &steps {
        let (c, pos) = (&step["completion"], &step["pos"]);
        let r = step["radius"].as_i64().expect("radius");
        assert_eq!(c["kind"], "cube");
        assert_eq!(r, 3, "the fixture authors radius 3");
        for i in 0..3 {
            let p = pos[i].as_i64().unwrap();
            assert_eq!(c["lo"][i].as_i64().unwrap(), p - r);
            assert_eq!(c["hi"][i].as_i64().unwrap(), p + r);
        }
    }
    let n = assert_agreement("blockout", &path, &ticks);
    assert_eq!(n, steps.len());
    assert!(n >= 2, "binding: {n} reach objective(s) examined");
}
