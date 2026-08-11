//! The bell remake's zone programs and what composition must not break.
//!
//! A vocabulary rule is judged on the shape it builds; a **zone** is judged on
//! what survives when several of those shapes are laid end to end. Three things
//! can go wrong that no piece-level gate could ever see, and each has a gate
//! here:
//!
//! * the pieces do not actually join, or they join and leave a way *round* the
//!   thing the zone exists for;
//! * a property a piece proved in a bare box stops holding in a bigger one — a
//!   sightline the zone put something into, or a hiding place that a much larger
//!   approach can now see into;
//! * a piece is silently **turned**, because every vocabulary rule chooses its
//!   own travel axis from the box it is handed (`z(Largest)`), and a piece run
//!   shorter than the zone is wide turns its wall across the route.
//!
//! Every gate below states its **binding count** — how many objects it actually
//! examined — and every one has been watched going red, either through a test
//! knob or through the refusal it is written as. A gate that matched nothing is
//! a finding, not a pass (`docs/reference/playtest-methodology.md` rule 1), so
//! the fixture tests pin those counts rather than leaving them to prose.

mod support;

use std::collections::BTreeSet;

use delvewright_grammar::block::BlockState;
use delvewright_grammar::compose::{AnchorRenames, entry, include_renaming};
use delvewright_grammar::geom::Axis;
use delvewright_grammar::ir::{Alternative, Node, Paint, Program, Size, Split};
use delvewright_grammar::library::bell::bell_tower::{CLIMB, RISE, SILL};
use delvewright_grammar::library::bell::cliff_road::{MIN_DROP, MIN_GULF};
use delvewright_grammar::library::elite_ground::MIN_RADIUS;
use delvewright_grammar::library::{
    barrow_shore, bell_tower, boulder_stair, broken_grate, causeway, chapel_ward, cistern_deep,
    cliff_road, disarm_stand, drowned_ward, elite_ground, far_side_bar, gate_ward, hall_keep,
    hearth_ward, stair_flight, watch_bay,
};
use delvewright_grammar::{Box3, ExpandOptions, Expansion, VoxelModel, expand};

use support::{
    connected, ends, expand_at, indexed, passable, reachable_with_fall, sees, solid, standable,
    standable_cells,
};

/// **Z0.** Barrow Shore: one open arena, in a box the campaign's size rather
/// than the piece fixture's.
const SHORE_REGION: Box3 = Box3::at_origin([19, 6, 24]);
/// The arena draws nothing from the seed; it is stated, not chosen.
const SHORE_SEED: u64 = 1;

/// **Z1.** The crag: three cells of gulf, the road, and the rock it is cut into,
/// over eight courses of drop.
const CLIFF_REGION: Box3 = Box3::at_origin([12, 12, 36]);
/// The seed the four-niche road is pinned to.
const CLIFF_SEED: u64 = 4;

/// **Z2.** Gatehouse and Outer Ward: the finished zone, and the longest of them.
/// Twelve cells of spill shaft at the exit end, then ten of boss threshold, ten
/// of sally-port junction, sixteen of boulder stair, ten of stair head, ten of
/// ward threshold and the remaining sixteen of gated passage. Nine-wide mainline
/// and an eleven-deep branch strip, for the two reasons Z6 has them; ten tall,
/// because the upper ward stands on a four-block plinth and still owes the watch
/// bay its headroom.
const WARD_REGION: Box3 = Box3::at_origin([20, 10, 84]);
/// Nothing in the gatehouse draws from the seed; it is stated, not chosen.
const WARD_SEED: u64 = 1;
/// How far the branch strip runs off Z2's mainline — the `strip_depth` default,
/// restated for the same reason `DEEP_STRIP` is.
const WARD_STRIP: i32 = 11;

/// **Z4.** Chapel Ward: eight cells of junction with the shortcut's strip beside
/// it, eight of rest ward, and the remaining ten of kitchen chute. Nine of the
/// sixteen columns are the branch strip — deeper than the junction is long, or
/// the bar turns along the mainline instead of across it — and nine tall so the
/// chute's ledge sits four blocks over a landing that still needs its own
/// headroom. The mainline is seven wide because the rest ward's nook asks for
/// it.
const CHAPEL_REGION: Box3 = Box3::at_origin([16, 9, 26]);
/// Nothing in the hub draws from the seed; it is stated, not chosen.
const CHAPEL_SEED: u64 = 1;

/// **Z5.** Keep: twelve cells of kitchen duct at the exit end, twelve of boss
/// threshold, twelve of gallery, twelve of stores, twelve of ward threshold and
/// the remaining sixteen of hall. Eleven tall, because the keep stands on the
/// duct's own four-block plinth and the gallery still needs a perch over its
/// pedestal.
const KEEP_REGION: Box3 = Box3::at_origin([11, 11, 76]);
/// The storeroom's tell is a seeded draw; this is the pinned fixture's seed.
const KEEP_SEED: u64 = 1;

/// **Z6.** Cistern Deep: twenty cells each of arena, sally port, grate wall and
/// dart gallery, and the remaining twenty of spill shaft. The mainline is
/// nineteen wide because the arena's flank margins ask for it, and the branch
/// strip twenty-one because it has to be deeper than the sally run is long — so
/// the box is forty across. Ten tall because the shaft's ledge sits four blocks
/// over a room that still needs its own headroom.
const DEEP_REGION: Box3 = Box3::at_origin([40, 10, 100]);
/// The grate row's break is a seeded draw; this is the pinned fixture's seed.
const DEEP_SEED: u64 = 1;

/// **Z3.** Drowned Lower Ward: twenty cells of lower ward, twenty of junction
/// with the shortcut's strip beside it, and the remaining twenty of flooded
/// crossing. Nineteen-wide mainline and twenty-one-deep strip for the same two
/// reasons Z6 has them — the arena's flank margins, and a branch that has to be
/// deeper than its junction is long. Ten tall because the causeway stacks a
/// berm, a gatehouse lane and the keeper's own headroom.
const DROWNED_REGION: Box3 = Box3::at_origin([40, 10, 60]);
/// Nothing in the drowned ward draws from the seed; it is stated, not chosen.
const DROWNED_SEED: u64 = 1;
/// The same as [`DEEP_STRIP`], for Z3.
const DROWNED_STRIP: i32 = 21;
/// How much of Z3's length the lower ward's arena takes — the `ward_run`
/// default, restated for the same reason `DROWNED_STRIP` is: the flank gate
/// reads cells off it rather than off the anchor arithmetic.
const DROWNED_WARD_RUN: i32 = 20;

/// How far the branch strip runs off Z6's mainline — the `strip_depth` default,
/// restated here because the gates read cells off it rather than off the anchor
/// arithmetic.
const DEEP_STRIP: i32 = 21;

/// The same, for Z4.
const CHAPEL_STRIP: i32 = 9;

/// **Z7.** Bell Tower: twenty cells each of boss ring, boss door, loft and
/// BF5's rope room, twenty-one of lift landing, and the remaining twenty-four of
/// ascent. The
/// mainline is nineteen wide because the Bellkeeper's ring asks for it, and the
/// branch strip twenty-two because it has to be deeper than the landing's run is
/// long — so the box is forty-one across. Fourteen tall: nine courses for the
/// flight to climb and the upper storey's own six on top of the eight it stands
/// on.
const TOWER_REGION: Box3 = Box3::at_origin([41, 14, 125]);
/// Nothing in the tower draws from the seed; it is stated, not chosen.
const TOWER_SEED: u64 = 1;
/// How far the branch strip runs off Z7's mainline — the `strip_depth` default,
/// restated for the same reason [`DEEP_STRIP`] is.
const TOWER_STRIP: i32 = 22;
/// How much of Z7's length the Bellkeeper's ring takes — the `ring_run` default,
/// restated because the flank gate reads cells off it.
const TOWER_RING_RUN: i32 = 20;

/// The odd barrel's block (`store_room`'s `barrel_unbanded` default).
const TELL_BLOCK: &str = "minecraft:spruce_log";

/// A zone, the box it is pinned in, and how a player crosses it.
///
/// `falls` is not a dial on how hard the gate tries: it says which movement
/// model is the *truthful* one for this zone. Three of the seven are flat, and a
/// walker crosses them with [`connected`]'s ±1 step. The other four have a drop
/// on the route — Z4 and Z6 are *entered* by stepping off a ledge, Z2 and Z5 are
/// *left* down one — so `connected` alone would call them severed and would be
/// measuring the wrong thing; they are crossed under
/// [`support::reachable_with_fall`] instead, and each of the four additionally
/// owes the negative claim, that the model's extra freedom still does not carry
/// a player back up.
struct ZoneFixture {
    program: Program,
    region: Box3,
    seed: u64,
    falls: bool,
}

fn zone(program: Program, region: Box3, seed: u64) -> ZoneFixture {
    ZoneFixture {
        program,
        region,
        seed,
        falls: false,
    }
}

fn zones() -> Vec<ZoneFixture> {
    vec![
        zone(barrow_shore(), SHORE_REGION, SHORE_SEED),
        zone(cliff_road(), CLIFF_REGION, CLIFF_SEED),
        ZoneFixture {
            falls: true,
            ..zone(gate_ward(), WARD_REGION, WARD_SEED)
        },
        zone(drowned_ward(), DROWNED_REGION, DROWNED_SEED),
        ZoneFixture {
            falls: true,
            ..zone(chapel_ward(), CHAPEL_REGION, CHAPEL_SEED)
        },
        ZoneFixture {
            falls: true,
            ..zone(hall_keep(), KEEP_REGION, KEEP_SEED)
        },
        ZoneFixture {
            falls: true,
            ..zone(cistern_deep(), DEEP_REGION, DEEP_SEED)
        },
        zone(bell_tower(), TOWER_REGION, TOWER_SEED),
    ]
}

// ---------------------------------------------------------------------------
// Promises every zone program owes, the same ones the rule library owes
// ---------------------------------------------------------------------------

/// A composed program is a program: every reference it makes resolves, which is
/// also the first thing that would break if [`delvewright_grammar::compose`]
/// missed a rewrite (a `call` or a `param` left pointing at the unprefixed name
/// is an `UnknownRule` / `UnknownParam` here, never a quietly different model).
#[test]
fn every_zone_program_is_structurally_valid() {
    for ZoneFixture { program, .. } in zones() {
        program
            .validate()
            .unwrap_or_else(|e| panic!("{}: {e}", program.name));
    }
}

/// ADR-0006 at zone scale: same program, region and seed, byte-identical model
/// *and* anchors.
#[test]
fn every_zone_expands_byte_identically_twice() {
    for ZoneFixture {
        program,
        region,
        seed,
        ..
    } in zones()
    {
        let a = expand_at(&program, region, seed);
        let b = expand_at(&program, region, seed);
        assert_eq!(
            a.model.canonical_bytes(),
            b.model.canonical_bytes(),
            "{}",
            program.name
        );
        assert_eq!(a.anchors, b.anchors, "{}", program.name);
    }
}

/// The composed program is still the authoring form: it serialises and parses
/// back to itself, prefixed names and all.
#[test]
fn every_zone_round_trips_through_json() {
    for ZoneFixture { program, .. } in zones() {
        let json = serde_json::to_string_pretty(&program).unwrap();
        let back: Program = serde_json::from_str(&json).unwrap();
        assert_eq!(back, program, "{}", program.name);
    }
}

/// A palette swap restyles a whole zone without moving a block — including the
/// roles that arrived with the included pieces, which is the claim that the
/// prefixed palette really is the one the rules read.
#[test]
fn every_zone_restyles_without_moving_a_block() {
    const SWATCH: &[&str] = &[
        "deepslate_bricks",
        "polished_blackstone",
        "cracked_nether_bricks",
        "warped_planks",
    ];
    for ZoneFixture {
        program: base,
        region,
        seed,
        ..
    } in zones()
    {
        let mut restyled = base.clone();
        // How many roles each zone binds is pinned in
        // `the_zone_fixtures_are_pinned`, where the rest of the binding counts
        // live; here it only has to be non-empty for the swap to mean anything,
        // and the byte-difference assertion at the bottom is what proves it did.
        let roles: Vec<String> = base.palette.keys().cloned().collect();
        assert!(!roles.is_empty(), "{} binds no roles", base.name);
        for (i, role) in roles.iter().enumerate() {
            restyled
                .set_role(
                    role,
                    Paint::Block(BlockState::simple(SWATCH[i % SWATCH.len()])),
                )
                .unwrap();
        }
        let plain = expand_at(&base, region, seed);
        let dark = expand_at(&restyled, region, seed);
        assert_eq!(
            plain.model.filled_cells(),
            dark.model.filled_cells(),
            "{} moved a block when it was restyled",
            base.name
        );
        assert_eq!(plain.anchors, dark.anchors, "{}", base.name);
        assert_ne!(
            plain.model.canonical_bytes(),
            dark.model.canonical_bytes(),
            "{}'s restyle changed nothing — the prefixed roles do not reach the blocks",
            base.name
        );
    }
}

/// **The first thing a zone owes: it is a route.** Pieces meet at an open seam
/// or they do not, and three sealed rooms in a row would satisfy every gate the
/// vocabulary has.
///
/// Binding: the standable cells of each zone — 438 (Z0), 40 (Z1), 655 (Z2),
/// 1100 (Z3), 165 (Z4), 677 (Z5), 2078 (Z6), 2172 (Z7).
#[test]
fn every_zone_is_walkable_end_to_end() {
    for ZoneFixture {
        program,
        region,
        seed,
        falls,
    } in zones()
    {
        let out = expand_at(&program, region, seed);
        let cells = standable_cells(&out.model);
        let (entry, exit) = ends(&out.model);
        assert!(
            !entry.is_empty() && !exit.is_empty(),
            "{}: nowhere to stand at the ends ({} standable cells)",
            program.name,
            cells.len()
        );
        let crossed = if falls {
            reachable_with_fall(&out.model, &cells, &entry, &exit)
        } else {
            connected(&cells, &entry, &exit)
        };
        assert!(
            crossed,
            "{}: the zone's pieces do not join into a route ({} standable cells, \
             {} at the entry end, {} at the exit end)",
            program.name,
            cells.len(),
            entry.len(),
            exit.len()
        );
    }
}

/// The pinned counts every gate below binds against, in one place, so that a
/// gate quietly binding to fewer objects than it used to is a red rather than a
/// silently weaker proof.
#[test]
fn the_zone_fixtures_are_pinned() {
    // Every role each zone inherited from the pieces it includes. A role that
    // silently stopped arriving would restyle nothing and break no other gate,
    // so the count is pinned rather than bounded.
    for (want, ZoneFixture { program, .. }) in [1, 3, 13, 7, 7, 13, 10, 12].into_iter().zip(zones())
    {
        assert_eq!(
            program.palette.len(),
            want,
            "{} binds {} palette roles",
            program.name,
            program.palette.len()
        );
    }

    let shore = expand_at(&barrow_shore(), SHORE_REGION, SHORE_SEED);
    assert_eq!(shore.anchors.len(), 1);
    assert_eq!(standable_cells(&shore.model).len(), 438);

    let cliff = expand_at(&cliff_road(), CLIFF_REGION, CLIFF_SEED);
    assert_eq!(indexed(&cliff.anchors, "niche").len(), 4);
    assert_eq!(indexed(&cliff.anchors, "niche-watch").len(), 4);
    assert_eq!(standable_cells(&cliff.model).len(), 40);

    // Z2 carries an anchor from each of its eight pieces, which is the cheapest
    // possible check that all eight expanded — and it is where Z2's rename is
    // read back: the sally port's bar answers to `sally-gate`, the portcullis the
    // bay watches still to `gate`.
    let ward = expand_at(&gate_ward(), WARD_REGION, WARD_SEED);
    let names: Vec<&str> = ward.anchors.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "anchor/alcove",
            "anchor/branch-door",
            "anchor/gate",
            "anchor/landing",
            "anchor/pocket-1",
            "anchor/pocket-2",
            "anchor/release",
            "anchor/run-head",
            "anchor/sally-gate",
            "anchor/spill",
            "anchor/stair-run",
            "anchor/threshold",
            "anchor/threshold-narrate",
            "anchor/unlock",
            "anchor/volley-slot",
            "anchor/watch",
        ]
    );
    assert_eq!(ward.anchors["anchor/gate"].declared_by, "gate/span_column");
    assert_eq!(
        ward.anchors["anchor/sally-gate"].declared_by,
        "sally/door_column"
    );
    assert_eq!(span_cells(&ward).len(), 21);
    assert_eq!(approach_cells(&ward).len(), 135);
    assert_eq!(standable_cells(&ward.model).len(), 655);
    assert_eq!(branch_near_room(&ward, WARD_STRIP).len(), 40);

    // Z3 carries one anchor from each of its four pieces, and it is where the
    // second rename in the library is read back: the crossing's keeper answers
    // to `keeper-elite`, the lower ward's own elite still to `elite`.
    let drowned = expand_at(&drowned_ward(), DROWNED_REGION, DROWNED_SEED);
    let names: Vec<&str> = drowned.anchors.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "anchor/branch-door",
            "anchor/causeway-head",
            "anchor/elite",
            "anchor/gate",
            "anchor/keeper-elite",
            "anchor/unlock",
        ]
    );
    assert_eq!(
        drowned.anchors["anchor/keeper-elite"].declared_by,
        "ward/post_column"
    );
    assert_eq!(
        drowned.anchors["anchor/elite"].declared_by,
        "ring/elite_column"
    );
    assert_eq!(standable_cells(&drowned.model).len(), 1100);
    assert_eq!(drowned_berm(&drowned).len(), 18);
    assert_eq!(branch_near_room(&drowned, DROWNED_STRIP).len(), 180);

    // Z4 carries one anchor from each of its four pieces — and, because the
    // hub's whole point is the branch, two of them come from the piece that is
    // *off* the mainline.
    let chapel = expand_at(&chapel_ward(), CHAPEL_REGION, CHAPEL_SEED);
    let names: Vec<&str> = chapel.anchors.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "anchor/branch-door",
            "anchor/gate",
            "anchor/hatch",
            "anchor/hearth",
            "anchor/landing",
            "anchor/unlock",
        ]
    );
    assert_eq!(standable_cells(&chapel.model).len(), 165);
    assert_eq!(branch_near_room(&chapel, CHAPEL_STRIP).len(), 24);

    let keep = expand_at(&hall_keep(), KEEP_REGION, KEEP_SEED);
    let names: Vec<&str> = keep.anchors.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "anchor/alcove",
            "anchor/bait",
            "anchor/bait-perch",
            "anchor/hall-door",
            "anchor/hatch",
            "anchor/landing",
            "anchor/perch-1",
            "anchor/perch-2",
            "anchor/perch-3",
            "anchor/perch-4",
            "anchor/store-line",
            "anchor/tell",
            "anchor/threshold",
            "anchor/threshold-narrate",
        ]
    );
    assert_eq!(indexed(&keep.anchors, "perch").len(), 4);
    assert_eq!(approach_cells(&keep).len(), 205);
    assert_eq!(standable_cells(&keep.model).len(), 677);

    // Z6 carries one anchor from each of its six pieces, which is also the
    // cheapest possible check that all six were actually expanded — and it is
    // where the rename is read back: the bar's gate answers to `sally-gate`, the
    // dart gallery's still to `gate`.
    let deep = expand_at(&cistern_deep(), DEEP_REGION, DEEP_SEED);
    let names: Vec<&str> = deep.anchors.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "anchor/branch-door",
            "anchor/elite",
            "anchor/gate",
            "anchor/grate-secret",
            "anchor/landing",
            "anchor/sally-gate",
            "anchor/spill",
            "anchor/unlock",
            "anchor/watch",
        ]
    );
    assert_eq!(
        deep.anchors["anchor/gate"].declared_by,
        "gallery/span_column"
    );
    assert_eq!(
        deep.anchors["anchor/sally-gate"].declared_by,
        "sally/door_column"
    );
    assert_eq!(span_cells(&deep).len(), 51);
    assert_eq!(standable_cells(&deep.model).len(), 2078);
    assert_eq!(branch_near_room(&deep, DEEP_STRIP).len(), 180);

    // Z7 carries an anchor from each of its six pieces, and it is the first zone
    // where two of them are indexed families — the flight's nine treads and the
    // loft's five perches. The count is pinned rather than the names alone: a
    // flight that laid a tread fewer would still name every stem in this list.
    let tower = expand_at(&bell_tower(), TOWER_REGION, TOWER_SEED);
    let names: Vec<&str> = tower
        .anchors
        .keys()
        .map(String::as_str)
        .filter(|n| !n.starts_with("anchor/stair-step-") && !n.starts_with("anchor/perch-"))
        .collect();
    assert_eq!(
        names,
        [
            "anchor/branch-door",
            "anchor/elite",
            "anchor/hall-door",
            "anchor/hearth",
            "anchor/lift-call-1",
            "anchor/lift-pit",
            "anchor/lift-station-1",
            "anchor/stair-foot",
            "anchor/stair-head",
            "anchor/threshold-narrate",
        ]
    );
    assert_eq!(tower.anchors.len(), 24, "the tower's whole anchor set");
    assert_eq!(
        indexed(&tower.anchors, "stair-step").len() as i64,
        CLIMB,
        "the flight laid a different number of treads than the zone's plinth is cut for"
    );
    assert_eq!(indexed(&tower.anchors, "perch").len(), 5);
    // One station and one call, deliberately: the tower is the *head* of the
    // shaft and the ride's far floor stands in another zone's box. A second
    // station here would mean a landing opening into the plinth.
    assert_eq!(indexed(&tower.anchors, "lift-station").len(), 1);
    assert_eq!(indexed(&tower.anchors, "lift-call").len(), 1);
    assert_eq!(standable_cells(&tower.model).len(), 2172);
    assert_eq!(tower_shaft_landings(&tower).len(), 1);
}

/// Z7's shaft landings: the strip's standable cells above the shaft floor — the
/// doorway sills a body steps through to board, and nothing else, because the
/// rest of the strip is the zone's own inert margin.
///
/// The upper bound is the strip's depth for the same reason
/// [`branch_near_room`]'s is: the strip is exactly the box the zone cut off its
/// own side and the mainline starts where it ends.
fn tower_shaft_landings(out: &Expansion) -> BTreeSet<[i32; 3]> {
    let pit = out.anchors["anchor/lift-pit"].pos;
    standable_cells(&out.model)
        .into_iter()
        .filter(|c| c[0] < TOWER_STRIP && c[1] > pit[1])
        .collect()
}

/// The branch's near room: the strip's standable cells on the mainline side of
/// the bar, read off the bar's own gate anchor rather than recomputed from the
/// zone's arithmetic.
///
/// The upper bound is the strip's depth, because the strip is exactly the box
/// the zone cut off its own side and the mainline starts where it ends.
fn branch_near_room(out: &Expansion, strip: i32) -> BTreeSet<[i32; 3]> {
    let gate = out.anchors[if out.anchors.contains_key("anchor/sally-gate") {
        "anchor/sally-gate"
    } else {
        "anchor/gate"
    }]
    .pos;
    standable_cells(&out.model)
        .into_iter()
        .filter(|c| c[0] > gate[0] && c[0] < strip)
        .collect()
}

// ---------------------------------------------------------------------------
// Z3 — the Drowned Lower Ward
// ---------------------------------------------------------------------------

/// The crossing itself: the berm's standable cells **over the flooded ward**,
/// read off `anchor/causeway-head`'s own column and stopping where the flood
/// does.
///
/// The last `guard_len` cells of the berm run under the gatehouse rather than
/// across the ward, and they are excluded for the same reason the piece-level
/// gate excludes them: they are the passage the player takes *under* the
/// keeper's floor, so the keeper cannot see them, and folding them into a gate
/// about the crossing would be folding a known blind spot into a green.
/// `tests/staging.rs` counts and bounds them at piece scale.
fn drowned_berm(out: &Expansion) -> BTreeSet<[i32; 3]> {
    let head = out.anchors["anchor/causeway-head"].pos;
    let ward = out
        .model
        .region()
        .positions()
        .filter(|&p| {
            out.model
                .get(p)
                .is_some_and(|b| b.name == "minecraft:water")
        })
        .map(|p| p[2])
        .min()
        .expect("the crossing built no flood at all");
    standable_cells(&out.model)
        .into_iter()
        .filter(|c| c[0] == head[0] && c[1] == head[1] && c[2] >= ward)
        .collect()
}

/// **Z3 gate 1.** The ward is a route in **both** directions under the plain
/// ±1 step — the stronger of the two movement models, which this zone owes
/// because it has no one-way hardware in it. The generic suite proves the
/// forward direction for every zone; what is here is the return leg (a zone
/// with a shortcut in it that could only be walked one way would be a lie) and
/// the teeth.
///
/// Teeth: `ward/berm_gate = 0` puts the guard post's plinth back, and the zone
/// is severed — while the crossing itself still walks its own length, so what
/// went red is the way past the gatehouse and not the gatehouse.
///
/// Binding: 1100 standable cells, 1 entry cell (the berm is one wide, which is
/// the point), 19 exit cells; 18 crossing cells still walked with the lane shut.
#[test]
fn the_drowned_ward_is_a_route_both_ways_and_only_through_the_gatehouse() {
    let out = expand_at(&drowned_ward(), DROWNED_REGION, DROWNED_SEED);
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    assert_eq!(entry.len(), 1, "the crossing is one wide: {entry:?}");
    assert!(
        connected(&cells, &entry, &exit) && connected(&cells, &exit, &entry),
        "the drowned ward is not walkable in both directions"
    );

    let mut plugged = drowned_ward();
    plugged.set_param("ward/berm_gate", 0).unwrap();
    let shut = expand_at(&plugged, DROWNED_REGION, DROWNED_SEED);
    let shut_cells = standable_cells(&shut.model);
    let (shut_entry, shut_exit) = ends(&shut.model);
    assert!(
        !shut_entry.is_empty() && !shut_exit.is_empty(),
        "the plugged zone has no faces, so the red below would be vacuous"
    );
    assert!(
        !connected(&shut_cells, &shut_entry, &shut_exit),
        "the guard post's plinth is back and the ward still crosses — either the \
         lane was never what carried the route, or something else does"
    );
    assert!(
        !reachable_with_fall(&shut.model, &shut_cells, &shut_entry, &shut_exit),
        "the plugged ward is crossable by stepping off something — the flood is \
         supposed to be the only thing beside the berm"
    );

    // ...and the control: with the plinth back, the crossing is still a
    // crossing. What the red above measured is the exit, not the causeway.
    let berm = drowned_berm(&shut);
    assert_eq!(berm.len(), 18, "the crossing with the lane shut");
    let far: BTreeSet<[i32; 3]> = berm
        .iter()
        .copied()
        .filter(|c| c[2] == berm.iter().map(|b| b[2]).max().unwrap())
        .collect();
    let near: BTreeSet<[i32; 3]> = berm
        .iter()
        .copied()
        .filter(|c| c[2] == berm.iter().map(|b| b[2]).min().unwrap())
        .collect();
    assert!(
        connected(&berm, &far, &near),
        "the plugged crossing does not even cross, so the red above is a broken fixture"
    );
}

/// **Z3 gate 2.** The keeper still commands the crossing after composition —
/// in the campaign's box, which is longer than the piece fixture's and puts the
/// whole zone's mass in the way.
///
/// Teeth: `ward/obstruct` stands one course of stone level with the keeper's own
/// floor and the same walk must lose cells, while the crossing stays walkable —
/// blindness, not impassability.
///
/// Binding: 18 crossing cells, 18 sightlines; teeth 18 cells with at least one
/// blind.
#[test]
fn the_keeper_sees_the_whole_drowned_crossing() {
    let out = expand_at(&drowned_ward(), DROWNED_REGION, DROWNED_SEED);
    let keeper = out.anchors["anchor/keeper-elite"].pos;
    let berm = drowned_berm(&out);
    assert_eq!(berm.len(), 18, "the crossing");
    for cell in &berm {
        if let Err(blocker) = sees(&out.model, keeper, *cell) {
            panic!(
                "after composition the keeper {keeper:?} cannot see the crossing cell \
                 {cell:?}: {blocker:?} is in the way"
            );
        }
    }

    let mut blinded = drowned_ward();
    blinded.set_param("ward/obstruct", 1).unwrap();
    let blocked = expand_at(&blinded, DROWNED_REGION, DROWNED_SEED);
    let keeper = blocked.anchors["anchor/keeper-elite"].pos;
    let berm = drowned_berm(&blocked);
    assert_eq!(
        berm.len(),
        18,
        "the obstructed crossing is still a crossing"
    );
    let blind = berm
        .iter()
        .filter(|c| sees(&blocked.model, keeper, **c).is_err())
        .count();
    assert!(
        blind > 0,
        "a pillar stands in the keeper's line and it sees all 18 cells anyway"
    );
    let cells = standable_cells(&blocked.model);
    let (entry, exit) = ends(&blocked.model);
    assert!(
        connected(&cells, &entry, &exit),
        "the obstruction sealed the ward, so the red above is impassability wearing \
         blindness's name"
    );
}

/// **Z3 gate 3.** The fight in the lower ward is still optional — bound over
/// **the arena's own run**, which is where the claim is true.
///
/// Z0 and Z6 bind the same claim across their whole zone. Z3 cannot and says so:
/// the causeway is a one-wide crossing, so no band of floor runs the length of
/// this zone at all, and asserting a zone-length bypass here would either be
/// false or be quietly re-scoped until it passed. The honest form is the arena's
/// own: two bands, each crossing the arena from its entry face to its exit face.
///
/// Teeth: `ring/seal_flank` at 1, 2 and 3 — the counted total drops to 1, 1, 0.
///
/// Binding: 2 routes at the default, over bands of 180 standable cells each; 3
/// teeth configurations.
#[test]
fn the_lower_ward_keeps_a_lane_on_each_side_of_its_fight() {
    for (knob, want) in [(0, 2), (1, 1), (2, 1), (3, 0)] {
        let mut program = drowned_ward();
        program.set_param("ring/seal_flank", knob).unwrap();
        let out = expand_at(&program, DROWNED_REGION, DROWNED_SEED);
        let elite = out.anchors["anchor/elite"].pos;
        assert_eq!(
            arena_flank_routes(&out, elite, DROWNED_WARD_RUN),
            want,
            "ring/seal_flank = {knob}"
        );
    }
}

/// The two bands of floor strictly west and strictly east of the lower ward's
/// engagement circle, each walked across the **arena's own run** rather than the
/// zone's — see [`the_lower_ward_keeps_a_lane_on_each_side_of_its_fight`].
///
/// The run is passed in from the zone's own `ward_run` default rather than
/// guessed, so a zone that re-sized its arena cannot silently shrink what this
/// examines.
fn arena_flank_routes(out: &Expansion, elite: [i32; 3], run: i32) -> usize {
    let cells: BTreeSet<[i32; 3]> = standable_cells(&out.model)
        .into_iter()
        .filter(|c| c[2] < run)
        .collect();
    [
        (c_lt(&cells, elite[0] - MIN_RADIUS as i32), "west"),
        (c_gt(&cells, elite[0] + MIN_RADIUS as i32), "east"),
    ]
    .into_iter()
    .filter(|(band, _)| {
        let entry: BTreeSet<[i32; 3]> = band.iter().copied().filter(|c| c[2] == run - 1).collect();
        let exit: BTreeSet<[i32; 3]> = band.iter().copied().filter(|c| c[2] == 0).collect();
        !entry.is_empty() && !exit.is_empty() && connected(band, &entry, &exit)
    })
    .count()
}

fn c_lt(cells: &BTreeSet<[i32; 3]>, x: i32) -> BTreeSet<[i32; 3]> {
    cells.iter().copied().filter(|c| c[0] < x).collect()
}

fn c_gt(cells: &BTreeSet<[i32; 3]>, x: i32) -> BTreeSet<[i32; 3]> {
    cells.iter().copied().filter(|c| c[0] > x).collect()
}

/// **Z3 gate 4.** The shortcut is sealed, its near room is reachable, and the
/// junction's doorway is the only way into it — the same four-part claim Z4 and
/// Z6 make, re-bound to this zone's own box.
///
/// Binding: 180 near-room cells; teeth `shortcut/unbarred` and
/// `junction/sealed`, each with a control that the ward itself still walks.
#[test]
fn the_drowned_wards_shortcut_is_sealed_and_reached_through_one_doorway() {
    let out = expand_at(&drowned_ward(), DROWNED_REGION, DROWNED_SEED);
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    let door = out.anchors["anchor/branch-door"].pos;
    let unlock: BTreeSet<[i32; 3]> = [out.anchors["anchor/unlock"].pos].into_iter().collect();

    assert!(
        connected(&cells, &entry, &exit),
        "the shortcut sealed the ward"
    );
    let near = branch_near_room(&out, DROWNED_STRIP);
    assert_eq!(near.len(), 180, "the branch's near room");
    assert!(
        connected(&cells, &entry, &near),
        "the ward cannot reach its own shortcut room"
    );
    assert!(
        !connected(&cells, &entry, &unlock),
        "the ward reaches {unlock:?} while the bar stands"
    );

    let mut open = drowned_ward();
    open.set_param("shortcut/unbarred", 1).unwrap();
    let opened = expand_at(&open, DROWNED_REGION, DROWNED_SEED);
    let open_cells = standable_cells(&opened.model);
    let (open_entry, _) = ends(&opened.model);
    let open_unlock: BTreeSet<[i32; 3]> =
        [opened.anchors["anchor/unlock"].pos].into_iter().collect();
    assert!(
        connected(&open_cells, &open_entry, &open_unlock),
        "drawing the bar did not open the branch, so the seal above proves nothing"
    );
    let cut: BTreeSet<[i32; 3]> = open_cells
        .iter()
        .copied()
        .filter(|c| c[0] != door[0] || c[2] != door[2])
        .collect();
    assert_eq!(
        cut.len(),
        open_cells.len() - 1,
        "exactly the doorway was cut"
    );
    assert!(
        !connected(&cut, &open_entry, &open_unlock),
        "with the junction's doorway plugged the unbarred branch is still reachable"
    );

    let mut sealed = drowned_ward();
    sealed.set_param("junction/sealed", 1).unwrap();
    let shut = expand_at(&sealed, DROWNED_REGION, DROWNED_SEED);
    let shut_cells = standable_cells(&shut.model);
    let (shut_entry, shut_exit) = ends(&shut.model);
    assert_eq!(
        shut_cells.len(),
        cells.len() - 1,
        "the doorway cell, and only it, is gone"
    );
    assert!(
        !connected(
            &shut_cells,
            &shut_entry,
            &branch_near_room(&shut, DROWNED_STRIP)
        ),
        "the junction's doorway was filled and the branch is still reachable"
    );
    assert!(
        connected(&shut_cells, &shut_entry, &shut_exit),
        "sealing the branch door sealed the ward, so the red above measures the \
         wrong thing"
    );
}

// ---------------------------------------------------------------------------
// Z0 — the Barrow Shore
// ---------------------------------------------------------------------------

/// How many of the two flank bands — the cells strictly west and strictly east
/// of the engagement circle — carry a route from the **zone's** entry face to
/// its exit face.
///
/// `elite_ground`'s own gate counts this inside the piece's box. At zone scale
/// the question is a different one: a bypass that stops at the arena's own seam
/// is not a bypass, so the walk runs the whole length of the zone, through
/// whatever else the zone put in the way. `falls` picks the movement model the
/// zone is truthful under (see [`ZoneFixture`]).
fn flank_routes(model: &VoxelModel, elite: [i32; 3], falls: bool) -> usize {
    let cells = standable_cells(model);
    let far = model.region().size[2] as i32 - 1;
    let west: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| c[0] < elite[0] - MIN_RADIUS as i32)
        .collect();
    let east: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| c[0] > elite[0] + MIN_RADIUS as i32)
        .collect();
    [west, east]
        .into_iter()
        .filter(|band| {
            let entry: BTreeSet<[i32; 3]> = band.iter().copied().filter(|c| c[2] == far).collect();
            let exit: BTreeSet<[i32; 3]> = band.iter().copied().filter(|c| c[2] == 0).collect();
            if entry.is_empty() || exit.is_empty() {
                return false;
            }
            if falls {
                reachable_with_fall(model, band, &entry, &exit)
            } else {
                connected(band, &entry, &exit)
            }
        })
        .count()
}

/// Z0's gate: the fight on the shore is on the route, and there is a way past it
/// on both sides — measured across the campaign's box, not the piece fixture's.
///
/// Z0 composes one piece, so this is `elite_ground`'s own claim re-bound to a
/// larger box rather than a new one; the module note on
/// [`delvewright_grammar::library::bell::barrow_shore`] says so plainly rather
/// than dressing it up.
///
/// Binding: 2 routes, over bands of 111 standable cells each.
#[test]
fn the_shore_keeps_a_lane_on_each_side_of_the_fight() {
    let out = expand_at(&barrow_shore(), SHORE_REGION, SHORE_SEED);
    let elite = out.anchors["anchor/elite"].pos;
    assert!(standable(&out.model, elite), "the elite cell {elite:?}");
    assert_eq!(
        flank_routes(&out.model, elite, false),
        2,
        "the shore does not carry a lane on each side of the circle — the opening \
         fight is compulsory"
    );
}

/// ...and it has teeth. `arena/seal_flank` walls off the west band, the east
/// band, or both, across the circle's own length — the shape a fog gate would
/// take — and the counted total has to drop.
///
/// The control is the second assertion: the zone stays walkable end to end
/// through the middle at every setting, so what went red is the bypass and not
/// the room.
#[test]
fn sealing_a_flank_of_the_shore_drops_its_route() {
    for (knob, want) in [(1, 1), (2, 1), (3, 0)] {
        let mut sealed = barrow_shore();
        sealed.set_param("arena/seal_flank", knob).unwrap();
        let out = expand_at(&sealed, SHORE_REGION, SHORE_SEED);
        let elite = out.anchors["anchor/elite"].pos;
        assert_eq!(
            flank_routes(&out.model, elite, false),
            want,
            "arena/seal_flank={knob} did not drop the zone-scale route count — the \
             gate proves nothing"
        );
        let cells = standable_cells(&out.model);
        let (entry, exit) = ends(&out.model);
        assert!(
            connected(&cells, &entry, &exit),
            "arena/seal_flank={knob} sealed the shore itself, so the route count is \
             measuring impassability rather than a missing bypass"
        );
    }
}

/// The frame guard, for the one zone where it collapses to a single comparison.
///
/// A zone whose only piece *is* the zone can only ask that the zone is longer
/// than it is wide, and its own `z(Largest)` has already normalised the box by
/// the time the guard runs — so a **square** box is the only shape left to
/// refuse. That is a narrow claim and the point of asserting it is that it stays
/// narrow: the module note explains why nothing wider can be caught here.
#[test]
fn a_square_shore_is_refused() {
    let square = Box3::at_origin([SHORE_REGION.size[0], SHORE_REGION.size[1], 19]);
    assert_eq!(square.size[0], square.size[2]);
    let err = expand(&barrow_shore(), square, &ExpandOptions::seeded(SHORE_SEED)).unwrap_err();
    assert!(
        err.to_string().contains("no alternative of rule"),
        "expected a refusal, got: {err}"
    );

    // The control: one cell longer and the same box builds, so what was refused
    // is the tie and not the size.
    let oblong = Box3::at_origin([square.size[0], square.size[1], 20]);
    expand_at(&barrow_shore(), oblong, SHORE_SEED);
}

// ---------------------------------------------------------------------------
// Z1 — the Cliff Road
// ---------------------------------------------------------------------------

/// The ledge lane, read off the anchors rather than off the `sea` parameter: a
/// recess is one cell in from the lane it opens onto, so the niches say where
/// the road is.
fn ledge_lane(out: &Expansion) -> i32 {
    let niches = indexed(&out.anchors, "niche");
    assert!(!niches.is_empty(), "the road declares no niches");
    let lanes: BTreeSet<i32> = niches.iter().map(|n| n[0] - 1).collect();
    assert_eq!(lanes.len(), 1, "the recesses are not all off one lane");
    *lanes.iter().next().unwrap()
}

/// Gate 1, at zone scale: the ledge is the only way along the road. The piece
/// proves this inside its own box; the zone could still have left a second lane
/// in the crag beside it, and this is where that would show.
///
/// Binding: 36 ledge cells of 40 standable, cut and re-walked.
#[test]
fn the_ledge_is_the_only_route_through_the_zone() {
    let out = expand_at(&cliff_road(), CLIFF_REGION, CLIFF_SEED);
    let cells = standable_cells(&out.model);
    let lane = ledge_lane(&out);
    let (entry, exit) = ends(&out.model);
    assert!(connected(&cells, &entry, &exit));

    let on_lane = cells.iter().filter(|c| c[0] == lane).count();
    assert_eq!(on_lane, 36, "the ledge should run the length of the zone");

    let cut: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[0] != lane).collect();
    assert!(
        !connected(&cut, &entry, &exit),
        "with the ledge deleted the zone still connects end to end — the crag left \
         a bypass, so the niches are decoration beside a safe road"
    );
}

/// Gate 2, and the reason this zone exists rather than a bare `cliff_path`: the
/// gulf. From every ledge cell, one step seaward is air, and under that air
/// there is nothing to land on for at least `fall` blocks.
///
/// Binding: 36 ledge cells × 3 seeds = 108 columns measured.
#[test]
fn every_ledge_cell_has_the_gulf_beside_it() {
    let fall = cliff_road().params["fall"];
    assert!(fall >= MIN_DROP);
    let mut measured = 0usize;
    for seed in [CLIFF_SEED, 11, 23] {
        let out = expand_at(&cliff_road(), CLIFF_REGION, seed);
        let lane = ledge_lane(&out);
        assert!(lane >= MIN_GULF as i32, "the gulf is {lane} wide");
        for cell in standable_cells(&out.model).iter().filter(|c| c[0] == lane) {
            let depth = clear_drop(&out, [cell[0] - 1, cell[1], cell[2]]);
            assert!(
                depth >= fall,
                "seed {seed}: from the ledge cell {cell:?} the drop is only {depth} \
                 blocks — a shove off this road is survivable, which is not the \
                 encounter the niche is for"
            );
            measured += 1;
        }
    }
    assert_eq!(measured, 108, "the gate examined {measured} columns");
}

/// ...and the gate has teeth. `ledge_shelf` lays a shelf across the gulf one
/// course under the road — the one mistake that turns a cliff into a step — and
/// the same measurement must go red while the road stays exactly as walkable.
#[test]
fn a_shelf_under_the_ledge_reds_the_drop_gate() {
    let mut caught = cliff_road();
    caught.set_param("ledge_shelf", 1).unwrap();
    let out = expand_at(&caught, CLIFF_REGION, CLIFF_SEED);
    let fall = caught.params["fall"];
    let lane = ledge_lane(&out);
    let cells = standable_cells(&out.model);

    let shallow: Vec<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| c[0] == lane && clear_drop(&out, [c[0] - 1, c[1], c[2]]) < fall)
        .collect();
    assert_eq!(
        shallow.len(),
        36,
        "a shelf was laid under the whole road and the drop gate still called it a \
         cliff — the gate proves nothing"
    );

    // The control: what went red is the drop and nothing else. The road is the
    // same road — same ledge cells, still walkable end to end.
    let plain = expand_at(&cliff_road(), CLIFF_REGION, CLIFF_SEED);
    assert_eq!(
        cells.iter().filter(|c| c[0] == lane).count(),
        standable_cells(&plain.model)
            .iter()
            .filter(|c| c[0] == lane)
            .count()
    );
    let (entry, exit) = ends(&out.model);
    assert!(connected(&cells, &entry, &exit));
}

/// Gate 3: every recess opens onto *that* lane. A niche whose mouth is a cell
/// back from the edge is an ambush next to a road, not on a ledge — the shove it
/// exists for has to have somewhere to send the player.
///
/// Binding: 4 niches (and their 4 watch cells).
#[test]
fn every_niche_opens_onto_the_ledge_over_the_gulf() {
    let out = expand_at(&cliff_road(), CLIFF_REGION, CLIFF_SEED);
    let model = &out.model;
    let lane = ledge_lane(&out);
    let niches = indexed(&out.anchors, "niche");
    let watches = indexed(&out.anchors, "niche-watch");
    assert_eq!(niches.len(), 4);
    assert_eq!(
        watches.len(),
        niches.len(),
        "every recess owes a watch cell"
    );

    for niche in &niches {
        let mouth = [lane, niche[1], niche[2]];
        assert!(standable(model, mouth), "the recess mouth {mouth:?}");
        assert!(
            solid(model, [niche[0] + 1, niche[1], niche[2]]),
            "the recess at {niche:?} is deeper than one cell"
        );
        assert!(
            passable(model, [lane - 1, niche[1], niche[2]]),
            "the cell the mouth {mouth:?} looks out over is not open air"
        );
    }
    for watch in &watches {
        assert_eq!(watch[0], lane, "a watch cell stands on the ledge");
        assert!(standable(model, *watch));
    }
}

/// How far a body shoved into `start` falls before something stops it.
fn clear_drop(out: &Expansion, start: [i32; 3]) -> i64 {
    let mut depth = 0;
    let mut y = start[1];
    while passable(&out.model, [start[0], y, start[2]]) {
        depth += 1;
        y -= 1;
    }
    depth
}

// ---------------------------------------------------------------------------
// Z2 — the Gatehouse
// ---------------------------------------------------------------------------

/// The standable cells of the hazard span, read off `anchor/gate` — `span` is a
/// parameter, so the geometry is measured rather than recomputed.
fn span_cells(out: &Expansion) -> Vec<[i32; 3]> {
    let gate = out.anchors["anchor/gate"].pos;
    standable_cells(&out.model)
        .into_iter()
        .filter(|c| (c[2] - gate[2]).abs() <= 1 && c[1] == gate[1])
        .collect()
}

/// Every standable cell on the approach side of the threshold wall — in a zone,
/// that is every cell the player crosses before the door, which is most of the
/// zone.
fn approach_cells(out: &Expansion) -> Vec<[i32; 3]> {
    let wall = out.anchors["anchor/threshold"].pos[2];
    standable_cells(&out.model)
        .into_iter()
        .filter(|c| c[2] > wall)
        .collect()
}

/// Gate 1: the portcullis is not optional. The zone connects end to end, and
/// with the span's cells deleted it does not — so the timed hazard is on the
/// route rather than beside it.
///
/// Both walks use the **fall** model, and that is the change the finished zone
/// forced: Z2 now ends in a spill shaft, so the player has an edge the plain
/// step never had, and a cut is only proved severing if it survives the more
/// permissive movement (`docs/reference/grammar.md` §5c, the same adversary use
/// Z6's span cut makes).
///
/// Binding: 21 span cells, 655 standable cells re-walked without them (634
/// after the cut).
#[test]
fn the_hazard_span_cannot_be_walked_round() {
    let out = expand_at(&gate_ward(), WARD_REGION, WARD_SEED);
    let cells = standable_cells(&out.model);
    let gate = out.anchors["anchor/gate"].pos;
    let (entry, exit) = ends(&out.model);
    assert!(reachable_with_fall(&out.model, &cells, &entry, &exit));

    let span = span_cells(&out);
    assert_eq!(span.len(), 21, "the span has cells to close");
    let cut: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| (c[2] - gate[2]).abs() > 1)
        .collect();
    assert_eq!(cut.len(), 634, "the span, and only it, was cut");
    assert!(
        !reachable_with_fall(&out.model, &cut, &entry, &exit),
        "with the portcullis span closed the gatehouse still connects end to end — \
         there is a way round the hazard, so nothing about its timing matters"
    );
}

/// Gate 2: the bay still sees the whole span once the zone is built around it.
/// `watch_bay` proves this in a bare box; the composition is a new box, and a
/// property re-proved on the assembled model is the only one a campaign can
/// rely on.
///
/// Binding: 21 span cells, each walked from the bay.
#[test]
fn the_bay_sees_the_whole_span_after_composition() {
    let out = expand_at(&gate_ward(), WARD_REGION, WARD_SEED);
    let watch = out.anchors["anchor/watch"].pos;
    assert!(standable(&out.model, watch), "the watch cell {watch:?}");
    let span = span_cells(&out);
    assert_eq!(span.len(), 21);
    for cell in &span {
        let standoff = (0..3).map(|i| (watch[i] - cell[i]).abs()).max().unwrap();
        assert!(
            standoff >= 6,
            "the bay stands only {standoff} from the span cell {cell:?}"
        );
        if let Err(blocker) = sees(&out.model, watch, *cell) {
            panic!("the bay cannot see the span cell {cell:?}: {blocker:?} is in the way");
        }
    }
}

/// ...and it has teeth, reached through the composed program's own parameter
/// namespace: `gate/obstruct` is the piece's knob, and that it is still
/// settable — and still reaches the same geometry — is half of what this test
/// proves.
#[test]
fn a_pillar_in_the_composed_line_reds_the_span_gate() {
    let mut blinded = gate_ward();
    blinded.set_param("gate/obstruct", 1).unwrap();
    let out = expand_at(&blinded, WARD_REGION, WARD_SEED);
    let watch = out.anchors["anchor/watch"].pos;
    let blind: Vec<[i32; 3]> = span_cells(&out)
        .into_iter()
        .filter(|c| sees(&out.model, watch, *c).is_err())
        .collect();
    assert_eq!(
        blind.len(),
        6,
        "a pillar was stood in the bay's line and every span cell still read as \
         visible — the gate proves nothing"
    );

    // The control: a pillar, not a plug. The gatehouse is still walkable, so
    // what the gate caught is blindness.
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    assert!(reachable_with_fall(&out.model, &cells, &entry, &exit));
}

/// Gate 3, and the one composition can most easily break: the alcove is blind
/// from **the whole zone**, not merely from the threshold piece's own approach.
/// The gatehouse gives the player 135 places to stand before the wall, against
/// the 54 the piece's own gate examined.
///
/// Binding: 135 approach cells.
#[test]
fn the_alcove_is_blind_from_the_whole_gatehouse() {
    let out = expand_at(&gate_ward(), WARD_REGION, WARD_SEED);
    let alcove = out.anchors["anchor/alcove"].pos;
    assert!(standable(&out.model, alcove), "the alcove cell {alcove:?}");
    let approach = approach_cells(&out);
    assert_eq!(approach.len(), 135, "the zone-scale approach set");
    for cell in &approach {
        if sees(&out.model, *cell, alcove).is_ok() {
            panic!(
                "the alcove {alcove:?} is visible from {cell:?}, {} cells up the \
                 gatehouse — a corner ambush the player reads on the approach will \
                 not happen",
                cell[2] - alcove[2]
            );
        }
    }
}

/// ...and it has teeth: `door/expose` widens the opening over the alcove's own
/// lane, and the zone-scale check must go red.
#[test]
fn a_widened_doorway_exposes_the_alcove_to_the_gatehouse() {
    let mut exposed = gate_ward();
    exposed.set_param("door/expose", 1).unwrap();
    let out = expand_at(&exposed, WARD_REGION, WARD_SEED);
    let alcove = out.anchors["anchor/alcove"].pos;
    let seen = approach_cells(&out)
        .into_iter()
        .filter(|c| sees(&out.model, *c, alcove).is_ok())
        .count();
    assert_eq!(
        seen, 121,
        "the doorway was widened straight over the alcove and the zone-scale \
         blindness check still reported it hidden — the gate proves nothing"
    );
}

/// Gate 4: the ward is a route **down**, and it is not a route back up. The
/// player walks in at the gate and leaves off the spill shaft's ledge, so the
/// whole upper ward — passage, threshold, stair, sally port, boss door — cannot
/// be re-entered from the ward below.
///
/// This is the claim the zone's **plinth** exists to make possible, and the one
/// that would break first if the plinth and the shaft's own `drop` ever drifted
/// apart: a plinth one block out and the seam at the ledge is a wall or a step.
///
/// Binding: 4 cells at the entry, 7 at the exit, 655 standable.
#[test]
fn the_gatehouse_is_a_route_down_and_not_back_up() {
    let out = expand_at(&gate_ward(), WARD_REGION, WARD_SEED);
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    assert_eq!(entry.len(), 4, "the gated passage at the way in");
    assert_eq!(exit.len(), 7, "the shaft's landing at the way out");
    assert_eq!(cells.len(), 655);

    assert!(
        reachable_with_fall(&out.model, &cells, &entry, &exit),
        "the ward does not reach its own spill shaft — the plinth and the shaft's \
         entry floor do not meet"
    );
    assert!(
        !connected(&cells, &exit, &entry),
        "the ward below climbs back into the gatehouse — the spill is a step"
    );
}

/// ...with the shaft's own teeth, and the plinth follows the drop it is told to
/// be: `shaft/drop = 2` shortens both, which is the only reason one notch can
/// bridge the gap at all.
#[test]
fn a_ladder_up_the_spill_reds_the_gatehouses_one_way_gate() {
    let mut rescued = gate_ward();
    rescued.set_param("shaft/rescue_ladder", 1).unwrap();
    rescued.set_param("shaft/drop", 2).unwrap();
    let out = expand_at(&rescued, WARD_REGION, WARD_SEED);
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    assert!(
        connected(&cells, &exit, &entry),
        "a ladder was notched up the shaft and the ward still read as one-way — the \
         gate proves nothing"
    );
    assert!(
        reachable_with_fall(&out.model, &cells, &entry, &exit),
        "the notched shaft stopped being a route down at all, so the red above is \
         measuring a broken zone rather than a rescued one"
    );

    let mut short = gate_ward();
    short.set_param("shaft/drop", 2).unwrap();
    let plain = expand_at(&short, WARD_REGION, WARD_SEED);
    let plain_cells = standable_cells(&plain.model);
    let (plain_entry, plain_exit) = ends(&plain.model);
    assert!(
        !connected(&plain_cells, &plain_exit, &plain_entry),
        "the shortened drop alone already let the ward climb back, so the teeth test \
         above proves nothing about the ladder"
    );
}

/// Gate 5: **the boulder release cannot be worked from the run it governs.**
/// `disarm_stand` proves this against its own fixture's lane; the zone re-binds
/// it against every standable cell of the composed ward that is not the stand
/// itself — 603 of them, including the whole worn tread the release is about.
///
/// Binding: 603 run cells examined, 0 in reach; 1 operating position, reachable
/// from the ward. Teeth: `stand/release_in_lane`.
#[test]
fn the_boulder_release_is_out_of_reach_of_the_composed_run() {
    let out = expand_at(&gate_ward(), WARD_REGION, WARD_SEED);
    let (run, operators) = ward_run_and_operators(&out);
    assert_eq!(run.len(), 603, "the run the release governs");
    assert_eq!(operators.len(), 1, "the stand's own operating position");

    let release = out.anchors["anchor/release"].pos;
    let touching = run.iter().filter(|c| in_reach(**c, release)).count();
    assert_eq!(
        touching, 0,
        "the release can be worked from inside the ward's own run — a boulder you \
         jam while standing in its lane is not a disarm"
    );

    let cells = standable_cells(&out.model);
    let (entry, _) = ends(&out.model);
    assert!(
        reachable_with_fall(&out.model, &cells, &entry, &operators),
        "the stand cannot be reached from the way in — a control nobody reaches is \
         absent, not safe"
    );
}

/// ...and it has teeth: `stand/release_in_lane` sets the mechanism into the
/// divider instead of the stand's outer wall, and the composed count of in-run
/// operating positions rises off zero.
#[test]
fn a_release_in_the_lane_reds_the_composed_disarm_gate() {
    let mut wrong = gate_ward();
    wrong.set_param("stand/release_in_lane", 1).unwrap();
    let out = expand_at(&wrong, WARD_REGION, WARD_SEED);
    let (run, _) = ward_run_and_operators(&out);
    assert_eq!(run.len(), 603, "the same run");
    let release = out.anchors["anchor/release"].pos;
    let touching = run.iter().filter(|c| in_reach(**c, release)).count();
    assert_eq!(
        touching, 1,
        "the mechanism was moved into the lane's own wall and the composed gate \
         still reported it out of reach — it proves nothing"
    );
}

/// The ward's run, and the positions the release can be worked from. The run is
/// **every standable cell but the stand's own two-wide pocket**: the hazard's
/// path is the whole lane, and a definition that only counted the stair's slice
/// would let a mechanism reachable from the lane beside it pass unnoticed.
fn ward_run_and_operators(out: &Expansion) -> (BTreeSet<[i32; 3]>, BTreeSet<[i32; 3]>) {
    let head = out.anchors["anchor/run-head"].pos;
    let release = out.anchors["anchor/release"].pos;
    let cells = standable_cells(&out.model);
    let stand: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| {
            c[0] > WARD_STRIP
                && c[0] <= WARD_STRIP + disarm_stand::STAND_WIDTH as i32
                && c[2] > head[2]
        })
        .collect();
    let run: BTreeSet<[i32; 3]> = cells.difference(&stand).copied().collect();
    let operators: BTreeSet<[i32; 3]> = stand
        .iter()
        .copied()
        .filter(|c| in_reach(*c, release))
        .collect();
    (run, operators)
}

/// Orthogonally touching at the same height: what "in reach" means for a hand on
/// a wall block.
fn in_reach(cell: [i32; 3], block: [i32; 3]) -> bool {
    cell[1] == block[1] && (cell[0] - block[0]).abs() + (cell[2] - block[2]).abs() == 1
}

/// Gate 6: the sally port is sealed, reachable, and reached through one doorway
/// — the same three claims Z4 and Z6 make about their own branches, on the zone
/// that taught the pattern its last consumer.
///
/// Binding: 655 standable cells, 40 of them the branch's near room, 1 the
/// doorway column that is cut and re-walked. Teeth: `sally/unbarred` (663
/// standable) and `tee/sealed` (654).
#[test]
fn the_gatehouses_sally_port_is_sealed_and_reached_through_one_doorway() {
    let out = expand_at(&gate_ward(), WARD_REGION, WARD_SEED);
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    let door = out.anchors["anchor/branch-door"].pos;
    let unlock: BTreeSet<[i32; 3]> = [out.anchors["anchor/unlock"].pos].into_iter().collect();

    assert!(reachable_with_fall(&out.model, &cells, &entry, &exit));
    let near = branch_near_room(&out, WARD_STRIP);
    assert_eq!(near.len(), 40, "the branch's near room");
    assert!(
        reachable_with_fall(&out.model, &cells, &entry, &near),
        "the ward cannot reach its own sally port — the junction's doorway opens \
         onto nothing"
    );
    assert!(
        !reachable_with_fall(&out.model, &cells, &entry, &unlock),
        "the ward reaches {unlock:?} while the bar stands — the shortcut has no far \
         side to earn"
    );

    let mut open = gate_ward();
    open.set_param("sally/unbarred", 1).unwrap();
    let opened = expand_at(&open, WARD_REGION, WARD_SEED);
    let open_cells = standable_cells(&opened.model);
    let (open_entry, _) = ends(&opened.model);
    let open_unlock: BTreeSet<[i32; 3]> =
        [opened.anchors["anchor/unlock"].pos].into_iter().collect();
    assert_eq!(open_cells.len(), 663, "the unbarred ward");
    assert!(
        reachable_with_fall(&opened.model, &open_cells, &open_entry, &open_unlock),
        "drawing the bar did not open the branch, so the seal above proves nothing"
    );
    let cut: BTreeSet<[i32; 3]> = open_cells
        .iter()
        .copied()
        .filter(|c| c[0] != door[0] || c[2] != door[2])
        .collect();
    assert_eq!(
        cut.len(),
        open_cells.len() - 1,
        "exactly the doorway was cut"
    );
    assert!(
        !reachable_with_fall(&opened.model, &cut, &open_entry, &open_unlock),
        "with the junction's doorway plugged the unbarred branch is still reachable"
    );

    let mut sealed = gate_ward();
    sealed.set_param("tee/sealed", 1).unwrap();
    let shut = expand_at(&sealed, WARD_REGION, WARD_SEED);
    let shut_cells = standable_cells(&shut.model);
    assert_eq!(
        shut_cells.len(),
        654,
        "the doorway cell, and only it, is gone"
    );
    let (shut_entry, shut_exit) = ends(&shut.model);
    let shut_near = branch_near_room(&shut, WARD_STRIP);
    assert_eq!(shut_near.len(), 40, "the sally port is still a room");
    assert!(
        !reachable_with_fall(&shut.model, &shut_cells, &shut_entry, &shut_near),
        "the junction's doorway was filled and the branch is still reachable — the \
         way in was never the doorway"
    );
    assert!(
        reachable_with_fall(&shut.model, &shut_cells, &shut_entry, &shut_exit),
        "sealing the branch door sealed the ward, so the red above is measuring an \
         impassable zone rather than an unreachable branch"
    );
}

// ---------------------------------------------------------------------------
// Z5 — the Great Hall and Keep
// ---------------------------------------------------------------------------

/// Gate 1: the doorway is the only way from the hall into the stores. Cut its
/// column and the keep is two rooms.
///
/// Binding: 205 approach cells, 471 cells behind the wall, 677 standable — and
/// the cut is re-walked under the fall model as well, because the finished keep
/// ends in a kitchen duct and a fall edge only ever adds routes.
#[test]
fn the_doorway_is_the_only_route_from_hall_to_stores() {
    let out = expand_at(&hall_keep(), KEEP_REGION, KEEP_SEED);
    let cells = standable_cells(&out.model);
    let threshold = out.anchors["anchor/threshold"].pos;
    let approach: BTreeSet<[i32; 3]> = approach_cells(&out).into_iter().collect();
    let inside: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| c[2] < threshold[2])
        .collect();
    assert_eq!(approach.len(), 205);
    assert_eq!(inside.len(), 471);
    assert!(
        connected(&cells, &approach, &inside),
        "the hall and the stores do not join"
    );

    let cut: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| c[0] != threshold[0] || c[2] != threshold[2])
        .collect();
    assert!(
        !connected(&cut, &approach, &inside),
        "with the doorway plugged the keep still lets a walker through — there is a \
         second way into the stores, so passing the alcove is optional"
    );
    assert!(
        !reachable_with_fall(&out.model, &cut, &approach, &inside),
        "the doorway is the only route on foot, but a fall carries a player past it"
    );
}

/// Gate 2: the rafters are still legible from the door of the assembled hall.
/// The hall in a zone is a different box from the hall in a fixture — shorter,
/// taller, with a wall at the far end — and the corbel form has to carry the
/// sightline in that box too.
///
/// Binding: 4 perches.
#[test]
fn every_perch_is_visible_from_the_hall_door_after_composition() {
    let out = expand_at(&hall_keep(), KEEP_REGION, KEEP_SEED);
    let door = out.anchors["anchor/hall-door"].pos;
    assert!(standable(&out.model, door), "the door cell {door:?}");
    let perches = indexed(&out.anchors, "perch");
    assert_eq!(perches.len(), 4, "the composed hall carries its rafters");
    for perch in &perches {
        assert!(standable(&out.model, *perch), "the perch cell {perch:?}");
        if let Err(blocker) = sees(&out.model, door, *perch) {
            panic!(
                "the doorway {door:?} cannot see the perch {perch:?}: {blocker:?} is in \
                 the way — a rafter with no silhouette is an ambush with no telegraph"
            );
        }
    }
}

/// ...and it has teeth: `hall/span_beams` closes the truss across the nave, and
/// the composed hall goes blind exactly as the bare one does.
#[test]
fn a_full_span_truss_blinds_the_composed_hall() {
    let mut blinded = hall_keep();
    blinded.set_param("hall/span_beams", 1).unwrap();
    let out = expand_at(&blinded, KEEP_REGION, KEEP_SEED);
    let door = out.anchors["anchor/hall-door"].pos;
    let blind: Vec<[i32; 3]> = indexed(&out.anchors, "perch")
        .into_iter()
        .filter(|p| sees(&out.model, door, *p).is_err())
        .collect();
    assert_eq!(
        blind.len(),
        3,
        "the truss was closed across the nave and the door still saw every perch — \
         the gate proves nothing"
    );

    // The control: the timbers are a ceiling, not a wall — the keep still walks
    // end to end, so what the gate caught is blindness.
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    assert!(reachable_with_fall(&out.model, &cells, &entry, &exit));
}

/// Gate 3: the alcove is blind from the whole hall — including from the
/// rafters, six blocks up, which no piece-level fixture had.
///
/// Binding: 205 approach cells, 4 of them perches.
#[test]
fn the_alcove_is_blind_from_the_whole_hall() {
    let out = expand_at(&hall_keep(), KEEP_REGION, KEEP_SEED);
    let alcove = out.anchors["anchor/alcove"].pos;
    let approach = approach_cells(&out);
    assert_eq!(approach.len(), 205);
    let perches: BTreeSet<[i32; 3]> = indexed(&out.anchors, "perch").into_iter().collect();
    assert_eq!(
        approach.iter().filter(|c| perches.contains(*c)).count(),
        4,
        "the approach set does not include the rafters, so the high sightlines are \
         not being checked at all"
    );
    for cell in &approach {
        if sees(&out.model, *cell, alcove).is_ok() {
            panic!("the alcove {alcove:?} is visible from {cell:?}");
        }
    }
}

/// ...with the same teeth as the gatehouse's, on a different approach set.
#[test]
fn a_widened_doorway_exposes_the_alcove_to_the_hall() {
    let mut exposed = hall_keep();
    exposed.set_param("door/expose", 1).unwrap();
    let out = expand_at(&exposed, KEEP_REGION, KEEP_SEED);
    let alcove = out.anchors["anchor/alcove"].pos;
    let seen = approach_cells(&out)
        .into_iter()
        .filter(|c| sees(&out.model, *c, alcove).is_ok())
        .count();
    assert_eq!(
        seen, 155,
        "the doorway was widened over the alcove and the hall-scale blindness check \
         still reported it hidden — the gate proves nothing"
    );
}

/// Gate 4: the stores hold exactly one tell in the composed zone.
///
/// `store_room` places it with a recursion — a rule that calls *itself* — so
/// this is also the gate that would catch an include that rewrote a rule's name
/// without rewriting its self-call, or that let two copies of the line run.
///
/// Binding: 8 seeds × the whole zone's cells searched for the tell block; 6
/// distinct positions among them.
#[test]
fn the_stores_hold_exactly_one_tell_in_the_composed_zone() {
    let program = hall_keep();
    let mut places = BTreeSet::new();
    for seed in 0..8u64 {
        let out = expand_at(&program, KEEP_REGION, seed);
        let tells: Vec<[i32; 3]> = KEEP_REGION
            .positions()
            .filter(|&p| {
                out.model
                    .get(p)
                    .is_some_and(|b| b.name.starts_with(TELL_BLOCK))
            })
            .collect();
        assert_eq!(tells.len(), 1, "seed {seed} laid {tells:?}");
        assert_eq!(out.anchors["anchor/tell"].pos, tells[0], "seed {seed}");
        places.insert(tells[0]);
    }
    assert!(
        places.len() >= 3,
        "8 seeds put the tell in {} places — a fixed tell is a landmark, not a tell",
        places.len()
    );
}

/// Gate 5: the keep is a route **down**, and not back up. The player walks in at
/// the hall and leaves down the kitchen duct — the same plinth construction Z2
/// uses, and the same pair of movement models.
///
/// Binding: 9 cells at the entry, 9 at the exit, 677 standable.
#[test]
fn the_keep_is_a_route_down_and_not_back_up() {
    let out = expand_at(&hall_keep(), KEEP_REGION, KEEP_SEED);
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    assert_eq!(entry.len(), 9, "the hall at the way in");
    assert_eq!(exit.len(), 9, "the duct's landing at the way out");
    assert_eq!(cells.len(), 677);
    assert!(
        reachable_with_fall(&out.model, &cells, &entry, &exit),
        "the keep does not reach its own kitchen duct — the plinth and the duct's \
         entry floor do not meet"
    );
    assert!(
        !connected(&cells, &exit, &entry),
        "the hub below climbs back into the keep — the duct is a step"
    );
}

/// ...with the duct's own teeth, the plinth shortening with the drop.
#[test]
fn a_ladder_up_the_kitchen_duct_reds_the_keeps_one_way_gate() {
    let mut rescued = hall_keep();
    rescued.set_param("duct/rescue_ladder", 1).unwrap();
    rescued.set_param("duct/drop", 2).unwrap();
    let out = expand_at(&rescued, KEEP_REGION, KEEP_SEED);
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    assert!(
        connected(&cells, &exit, &entry),
        "a ladder was notched up the duct and the keep still read as one-way — the \
         gate proves nothing"
    );
    assert!(
        reachable_with_fall(&out.model, &cells, &entry, &exit),
        "the notched duct stopped being a route down at all, so the red above is \
         measuring a broken zone rather than a rescued one"
    );

    let mut short = hall_keep();
    short.set_param("duct/drop", 2).unwrap();
    let plain = expand_at(&short, KEEP_REGION, KEEP_SEED);
    let plain_cells = standable_cells(&plain.model);
    let (plain_entry, plain_exit) = ends(&plain.model);
    assert!(
        !connected(&plain_cells, &plain_exit, &plain_entry),
        "the shortened drop alone already let the keep climb back, so the teeth test \
         above proves nothing about the ladder"
    );
}

/// Gate 6: **the lure's watcher is legible from everywhere the lure is** — the
/// composed form of `bait_stand`'s own gate, over the gallery's whole run rather
/// than a bare fixture's.
///
/// The claim is bound to the gallery's own run **and that is a finding, not a
/// convenience**. Over the whole zone 242 cells see the pedestal and only 212
/// see the body over it: the 30 that differ are all *in another room*, looking
/// through the ambush door's two-block opening, which shows the floor and hides
/// anything four blocks up. That is the doorway's geometry, not the stand's, and
/// the decision the pattern is about — take it or leave it — is made in the
/// gallery. Re-scoping a gate until it passes is the vacuity these counts exist
/// to catch, so the number that failed is written down here rather than dropped.
///
/// Binding: 144 gallery cells, all 144 seeing both. Teeth: `gallery/canopy`.
#[test]
fn the_lures_watcher_is_legible_from_the_composed_gallery() {
    let out = expand_at(&hall_keep(), KEEP_REGION, KEEP_SEED);
    let (bait, perch, gallery) = gallery_view(&out);
    assert_eq!(gallery.len(), 144, "the gallery's own run");
    let mut lure_seen = 0;
    for cell in &gallery {
        if sees(&out.model, *cell, bait).is_ok() {
            lure_seen += 1;
            if let Err(blocker) = sees(&out.model, *cell, perch) {
                panic!(
                    "from {cell:?} the composed gallery shows the lure and hides the \
                     body over it ({blocker:?} is in the way)"
                );
            }
        }
    }
    assert_eq!(lure_seen, 144, "the lure is visible from the whole gallery");
}

/// ...and it has teeth: `gallery/canopy` hangs a valance in front of the perch,
/// and the lure's own count does not move — so what the gate catches is a hidden
/// ambusher, not a walled-off room.
#[test]
fn a_canopy_in_the_composed_gallery_hides_its_watcher() {
    let mut hidden = hall_keep();
    hidden.set_param("gallery/canopy", 1).unwrap();
    let out = expand_at(&hidden, KEEP_REGION, KEEP_SEED);
    let (bait, perch, gallery) = gallery_view(&out);
    assert_eq!(gallery.len(), 144, "the same gallery run");
    let lure_seen = gallery
        .iter()
        .filter(|c| sees(&out.model, **c, bait).is_ok())
        .count();
    let watcher_seen = gallery
        .iter()
        .filter(|c| sees(&out.model, **c, perch).is_ok())
        .count();
    assert_eq!(
        lure_seen, 144,
        "the valance moved the lure's own visibility"
    );
    assert_eq!(
        watcher_seen, 0,
        "a valance was hung in front of the perch and the composed co-visibility \
         gate still read it as legible — the gate proves nothing"
    );
}

/// The pedestal, the perch over it, and the gallery's own standable run: between
/// the perch and the storeroom's barrel line, which is the room the decision is
/// made in.
fn gallery_view(out: &Expansion) -> ([i32; 3], [i32; 3], Vec<[i32; 3]>) {
    let bait = out.anchors["anchor/bait"].pos;
    let perch = out.anchors["anchor/bait-perch"].pos;
    let stores = out.anchors["anchor/store-line"].pos;
    let gallery = standable_cells(&out.model)
        .into_iter()
        .filter(|c| c[2] > perch[2] + 1 && c[2] < stores[2])
        .collect();
    (bait, perch, gallery)
}

// ---------------------------------------------------------------------------
// Z6 — the Cistern Deep
// ---------------------------------------------------------------------------

/// Gate 1: the cistern is a route **downward**, and it is not a route back up.
///
/// `drop_shaft` proves this between its own two anchors in a bare box. The zone
/// claim is far larger and is the one a campaign actually needs: the whole
/// gallery above the drop — the ledge, the landing, the bay, the span, the lane
/// — cannot be re-entered from anywhere in the deep. Two seams and three other
/// pieces sit between those two sets, and any of them could have left a step.
///
/// The two directions use different models on purpose (see
/// [`support::reachable_with_fall`]): down under walk-and-fall, up under the
/// plain step, because a fall edge only ever points down and proving the
/// negative under the generous model would be circular.
///
/// Binding: 17 cells at the entry ledge, 19 at the exit, 2078 standable.
#[test]
fn the_cistern_is_a_route_down_and_not_back_up() {
    let out = expand_at(&cistern_deep(), DEEP_REGION, DEEP_SEED);
    let cells = standable_cells(&out.model);
    let (ledge, floor) = ends(&out.model);
    assert_eq!(ledge.len(), 17, "the entry ledge");
    assert_eq!(floor.len(), 19, "the cistern floor at the exit");
    assert_eq!(cells.len(), 2078);

    assert!(
        reachable_with_fall(&out.model, &cells, &ledge, &floor),
        "the spill does not reach the bottom of the cistern — the pieces below the \
         shaft do not join"
    );
    assert!(
        !connected(&cells, &floor, &ledge),
        "the deep walks back up to the gallery — the drop is a step, and everything \
         past it can be skipped by going round"
    );
}

/// ...and it has teeth, through the shaft's own knob reached in the composed
/// program's parameter namespace. `shaft/rescue_ladder` notches every column of
/// the entry floor but the brink's own, and — paired with a short `shaft/drop`,
/// exactly as the piece's teeth test pairs them — the climb back has to be
/// found.
///
/// Two controls, because this knob changes two things. The zone must still be a
/// route downward (so what went red is the return, not the structure), and the
/// short drop **on its own** must still be one-way (so what went red is the
/// ladder, not the shortened fall).
#[test]
fn a_ladder_up_the_shaft_reds_the_one_way_gate() {
    let mut rescued = cistern_deep();
    rescued.set_param("shaft/rescue_ladder", 1).unwrap();
    rescued.set_param("shaft/drop", 2).unwrap();
    let out = expand_at(&rescued, DEEP_REGION, DEEP_SEED);
    let cells = standable_cells(&out.model);
    let (ledge, floor) = ends(&out.model);
    assert!(
        connected(&cells, &floor, &ledge),
        "a ladder was notched up the whole shaft and the zone still read as one-way \
         — the gate proves nothing"
    );
    assert!(
        reachable_with_fall(&out.model, &cells, &ledge, &floor),
        "the notched shaft stopped being a route down at all, so the red above is \
         measuring a broken zone rather than a rescued one"
    );

    let mut short = cistern_deep();
    short.set_param("shaft/drop", 2).unwrap();
    let plain = expand_at(&short, DEEP_REGION, DEEP_SEED);
    let plain_cells = standable_cells(&plain.model);
    let (plain_ledge, plain_floor) = ends(&plain.model);
    assert!(
        !connected(&plain_cells, &plain_floor, &plain_ledge),
        "the shortened drop alone already let the deep climb back, so the teeth test \
         above proves nothing about the ladder"
    );
}

/// Gate 2: the volley span cannot be walked round — **or fallen past**.
///
/// This is the new risk a drop brings to a zone, and the reason it is not Z2's
/// gate wearing a different name. `watch_bay` and `gate_ward` both prove the
/// span is unavoidable under a walker's ±1 step; here the player arrives by
/// falling, and a fall edge only ever *adds* routes. A hazard that a walker
/// cannot bypass may still be one a faller drops straight past, so the cut is
/// re-walked under the permissive model.
///
/// Binding: 51 span cells deleted, 2078 standable re-walked without them.
/// Control: the same walk with the span present connects.
#[test]
fn the_volley_span_cannot_be_walked_round_or_fallen_past() {
    let out = expand_at(&cistern_deep(), DEEP_REGION, DEEP_SEED);
    let cells = standable_cells(&out.model);
    let gate = out.anchors["anchor/gate"].pos;
    let (ledge, floor) = ends(&out.model);
    assert!(reachable_with_fall(&out.model, &cells, &ledge, &floor));

    let span = span_cells(&out);
    assert_eq!(span.len(), 51, "the span has cells to close");
    let cut: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| (c[2] - gate[2]).abs() > 1)
        .collect();
    assert!(
        !reachable_with_fall(&out.model, &cut, &ledge, &floor),
        "with the volley span closed the cistern still reaches its own floor — the \
         hazard is beside the route rather than on it, or the drop lands past it"
    );
}

/// Gate 3: the fight at the bottom of the cistern is still optional, across the
/// whole zone — a route from the ledge the player falls off to the way out that
/// never enters the engagement circle, on each side.
///
/// Binding: 2 routes, over bands of 767 and 411 standable cells. The west band
/// is the larger only because the branch strip's own rooms fall inside it; they
/// span the sally run alone and so carry no route, which the counts under
/// `arena/seal_flank` below make good on.
#[test]
fn the_deep_keeps_a_lane_on_each_side_of_the_fight() {
    let out = expand_at(&cistern_deep(), DEEP_REGION, DEEP_SEED);
    let elite = out.anchors["anchor/elite"].pos;
    assert_eq!(
        flank_routes(&out.model, elite, true),
        2,
        "the cistern does not carry a lane past the elite's ground — a fight at the \
         bottom of a one-way zone with no way round it is a wall"
    );
}

/// ...and the same teeth as the shore's, on a zone the walk has to fall through
/// to reach the bands at all.
#[test]
fn sealing_a_flank_of_the_deep_drops_its_route() {
    for (knob, want) in [(1, 1), (2, 1), (3, 0)] {
        let mut sealed = cistern_deep();
        sealed.set_param("arena/seal_flank", knob).unwrap();
        let out = expand_at(&sealed, DEEP_REGION, DEEP_SEED);
        let elite = out.anchors["anchor/elite"].pos;
        assert_eq!(
            flank_routes(&out.model, elite, true),
            want,
            "arena/seal_flank={knob} did not drop the zone-scale route count — the \
             gate proves nothing"
        );
        let cells = standable_cells(&out.model);
        let (ledge, floor) = ends(&out.model);
        assert!(
            reachable_with_fall(&out.model, &cells, &ledge, &floor),
            "arena/seal_flank={knob} sealed the cistern itself, so the route count is \
             measuring impassability rather than a missing bypass"
        );
    }
}

/// Z6 gate 4: the sally port is sealed, it is reachable, and the way in is the
/// junction's own doorway.
///
/// Three claims, and the first is the one that separates a shortcut from a wall:
/// **the cistern still crosses with the bar standing.** That is exactly the
/// assertion a `far_side_bar` laid *in* the piece run fails, and it is why the
/// branch exists at all.
///
/// Binding: 2078 standable cells, 180 of them the branch's near room, 1 the
/// doorway column that is cut and re-walked. Teeth: `sally/unbarred` opens the
/// bar (2096 standable) and the same walk reaches `anchor/unlock`; plugging the
/// doorway column in *that* model makes it unreachable again.
#[test]
fn the_cistern_sally_port_is_sealed_and_opens_only_from_its_far_side() {
    let out = expand_at(&cistern_deep(), DEEP_REGION, DEEP_SEED);
    let cells = standable_cells(&out.model);
    let (ledge, floor) = ends(&out.model);
    let door = out.anchors["anchor/branch-door"].pos;
    let unlock: BTreeSet<[i32; 3]> = [out.anchors["anchor/unlock"].pos].into_iter().collect();

    // 1. The control: the mainline is still a route with the bar standing.
    assert!(
        reachable_with_fall(&out.model, &cells, &ledge, &floor),
        "the sally port sealed the cistern itself — a branch that severs the chain \
         it hangs off is a `far_side_bar` in the run wearing a different name"
    );

    // 2. The near side reaches the branch, and stops at the bar.
    let near = branch_near_room(&out, DEEP_STRIP);
    assert_eq!(near.len(), 180, "the branch's near room");
    assert!(
        reachable_with_fall(&out.model, &cells, &floor, &near),
        "the cistern cannot reach the sally port at all — the junction's doorway \
         opens onto nothing"
    );
    assert!(
        !reachable_with_fall(&out.model, &cells, &floor, &unlock),
        "the cistern reaches {unlock:?} while the bar stands — the shortcut has no \
         far side to earn"
    );

    // 3. Drawing the bar connects them, through exactly the junction's doorway.
    let mut open = cistern_deep();
    open.set_param("sally/unbarred", 1).unwrap();
    let opened = expand_at(&open, DEEP_REGION, DEEP_SEED);
    let open_cells = standable_cells(&opened.model);
    let (_, open_floor) = ends(&opened.model);
    let open_unlock: BTreeSet<[i32; 3]> =
        [opened.anchors["anchor/unlock"].pos].into_iter().collect();
    assert_eq!(open_cells.len(), 2096, "the unbarred cistern");
    assert!(
        reachable_with_fall(&opened.model, &open_cells, &open_floor, &open_unlock),
        "drawing the bar did not open the branch, so the seal above proves nothing"
    );
    let cut: BTreeSet<[i32; 3]> = open_cells
        .iter()
        .copied()
        .filter(|c| c[0] != door[0] || c[2] != door[2])
        .collect();
    assert_eq!(
        cut.len(),
        open_cells.len() - 1,
        "exactly the doorway column was cut"
    );
    assert!(
        !reachable_with_fall(&opened.model, &cut, &open_floor, &open_unlock),
        "with the junction's doorway plugged the unbarred branch is still reachable \
         — the way in is a seam that never closed, not the doorway the tee declares"
    );
}

/// ...and the branch has teeth of its own: `tee/sealed` fills the doorway and
/// nothing else.
///
/// Binding: 2077 standable cells (one fewer than the open fixture: the doorway),
/// 180 of them the branch's near room, still a room and now unreachable.
#[test]
fn sealing_the_cisterns_junction_makes_its_sally_port_unreachable() {
    let mut sealed = cistern_deep();
    sealed.set_param("tee/sealed", 1).unwrap();
    let out = expand_at(&sealed, DEEP_REGION, DEEP_SEED);
    let cells = standable_cells(&out.model);
    assert_eq!(cells.len(), 2077, "the doorway cell, and only it, is gone");
    let (ledge, floor) = ends(&out.model);

    let near = branch_near_room(&out, DEEP_STRIP);
    assert_eq!(near.len(), 180, "the sally port is still a room");
    assert!(
        !reachable_with_fall(&out.model, &cells, &floor, &near),
        "the junction's doorway was filled and the sally port is still reachable — \
         the way in was never the doorway, so the gate above proves nothing"
    );
    assert!(
        reachable_with_fall(&out.model, &cells, &ledge, &floor),
        "sealing the branch door sealed the cistern, so the red above is measuring \
         an impassable zone rather than an unreachable branch"
    );
}

// ---------------------------------------------------------------------------
// Z4 — the Chapel Ward
// ---------------------------------------------------------------------------

/// Z4 gate 1: the hub is a route, and only downward — the same pair of claims Z6
/// owes, for the same reason and against the same two movement models. The
/// player arrives off the keep's kitchen duct and cannot climb back up it.
///
/// Binding: 5 cells at the entry ledge, 5 at the exit, 165 standable.
#[test]
fn the_hub_is_a_route_down_and_not_back_up() {
    let out = expand_at(&chapel_ward(), CHAPEL_REGION, CHAPEL_SEED);
    let cells = standable_cells(&out.model);
    let (ledge, exit) = ends(&out.model);
    assert_eq!(ledge.len(), 5, "the duct's entry ledge");
    assert_eq!(exit.len(), 5, "the junction's lane at the way out");
    assert_eq!(cells.len(), 165);

    assert!(
        reachable_with_fall(&out.model, &cells, &ledge, &exit),
        "the chute does not reach the hub floor — the pieces below it do not join"
    );
    assert!(
        !connected(&cells, &exit, &ledge),
        "the hub walks back up the kitchen duct — the drop is a step, and the keep \
         above it can be re-entered from the rest ward"
    );
}

/// ...with the chute's own teeth, reached through the composed program's
/// parameter namespace, and the same two controls Z6's copy carries: the zone
/// must still be a route downward, and the shortened drop on its own must still
/// be one-way.
#[test]
fn a_ladder_up_the_chute_reds_the_hubs_one_way_gate() {
    let mut rescued = chapel_ward();
    rescued.set_param("chute/rescue_ladder", 1).unwrap();
    rescued.set_param("chute/drop", 2).unwrap();
    let out = expand_at(&rescued, CHAPEL_REGION, CHAPEL_SEED);
    let cells = standable_cells(&out.model);
    let (ledge, exit) = ends(&out.model);
    assert!(
        connected(&cells, &exit, &ledge),
        "a ladder was notched up the whole duct and the hub still read as one-way — \
         the gate proves nothing"
    );
    assert!(
        reachable_with_fall(&out.model, &cells, &ledge, &exit),
        "the notched duct stopped being a route down at all, so the red above is \
         measuring a broken zone rather than a rescued one"
    );

    let mut short = chapel_ward();
    short.set_param("chute/drop", 2).unwrap();
    let plain = expand_at(&short, CHAPEL_REGION, CHAPEL_SEED);
    let plain_cells = standable_cells(&plain.model);
    let (plain_ledge, plain_exit) = ends(&plain.model);
    assert!(
        !connected(&plain_cells, &plain_exit, &plain_ledge),
        "the shortened drop alone already let the hub climb back, so the teeth test \
         above proves nothing about the ladder"
    );
}

/// Z4 gates 2 and 3: the hub's shortcut is sealed, and the branch is entered
/// through the junction's doorway and nowhere else.
///
/// This is the gate the hub exists for. A hub is a junction with something
/// hanging off it, and before `tee_passage` the seam could only chain — so this
/// is also the smallest complete statement of what the closed seam limit bought.
///
/// Binding: 165 standable cells, 24 of them the branch's near room, 1 the
/// doorway column that is cut and re-walked. Teeth: `shortcut/unbarred` (171
/// standable) and `junction/sealed` (164).
#[test]
fn the_hubs_shortcut_is_sealed_and_reached_through_one_doorway() {
    let out = expand_at(&chapel_ward(), CHAPEL_REGION, CHAPEL_SEED);
    let cells = standable_cells(&out.model);
    let (ledge, exit) = ends(&out.model);
    let door = out.anchors["anchor/branch-door"].pos;
    let unlock: BTreeSet<[i32; 3]> = [out.anchors["anchor/unlock"].pos].into_iter().collect();

    assert!(
        reachable_with_fall(&out.model, &cells, &ledge, &exit),
        "the shortcut sealed the hub itself"
    );
    let near = branch_near_room(&out, CHAPEL_STRIP);
    assert_eq!(near.len(), 24, "the branch's near room");
    assert!(
        reachable_with_fall(&out.model, &cells, &ledge, &near),
        "the hub cannot reach its own shortcut room — the junction's doorway opens \
         onto nothing"
    );
    assert!(
        !reachable_with_fall(&out.model, &cells, &ledge, &unlock),
        "the hub reaches {unlock:?} while the bar stands — the shortcut has no far \
         side to earn"
    );

    let mut open = chapel_ward();
    open.set_param("shortcut/unbarred", 1).unwrap();
    let opened = expand_at(&open, CHAPEL_REGION, CHAPEL_SEED);
    let open_cells = standable_cells(&opened.model);
    let (open_ledge, _) = ends(&opened.model);
    let open_unlock: BTreeSet<[i32; 3]> =
        [opened.anchors["anchor/unlock"].pos].into_iter().collect();
    assert_eq!(open_cells.len(), 171, "the unbarred hub");
    assert!(
        reachable_with_fall(&opened.model, &open_cells, &open_ledge, &open_unlock),
        "drawing the bar did not open the branch, so the seal above proves nothing"
    );
    let cut: BTreeSet<[i32; 3]> = open_cells
        .iter()
        .copied()
        .filter(|c| c[0] != door[0] || c[2] != door[2])
        .collect();
    assert_eq!(
        cut.len(),
        open_cells.len() - 1,
        "exactly the doorway was cut"
    );
    assert!(
        !reachable_with_fall(&opened.model, &cut, &open_ledge, &open_unlock),
        "with the junction's doorway plugged the unbarred branch is still reachable"
    );

    let mut sealed = chapel_ward();
    sealed.set_param("junction/sealed", 1).unwrap();
    let shut = expand_at(&sealed, CHAPEL_REGION, CHAPEL_SEED);
    let shut_cells = standable_cells(&shut.model);
    assert_eq!(
        shut_cells.len(),
        164,
        "the doorway cell, and only it, is gone"
    );
    let (shut_ledge, shut_exit) = ends(&shut.model);
    let shut_near = branch_near_room(&shut, CHAPEL_STRIP);
    assert_eq!(shut_near.len(), 24, "the shortcut is still a room");
    assert!(
        !reachable_with_fall(&shut.model, &shut_cells, &shut_ledge, &shut_near),
        "the junction's doorway was filled and the branch is still reachable — the \
         way in was never the doorway"
    );
    assert!(
        reachable_with_fall(&shut.model, &shut_cells, &shut_ledge, &shut_exit),
        "sealing the branch door sealed the hub, so the red above is measuring an \
         impassable zone rather than an unreachable branch"
    );
}

/// The branch box has to be **deeper than the junction is long**, or
/// `far_side_bar`'s own `z(Largest)` lays its wall along the mainline instead of
/// across it — a shortcut that seals the hub. The zone refuses both ways round
/// rather than building the turned shape.
///
/// Binding: 2 refusals. Control: the same box builds at the defaults, so what is
/// refused is the ratio and not the region.
#[test]
fn a_branch_no_deeper_than_its_junction_is_refused() {
    for (knob, value) in [("strip_depth", 8), ("junction_run", 9)] {
        let mut bad = chapel_ward();
        bad.set_param(knob, value).unwrap();
        let err = expand(&bad, CHAPEL_REGION, &ExpandOptions::seeded(CHAPEL_SEED)).unwrap_err();
        assert!(
            err.to_string().contains("no alternative of rule"),
            "{knob}={value}: expected a refusal, got: {err}"
        );
    }
    expand_at(&chapel_ward(), CHAPEL_REGION, CHAPEL_SEED);
}

/// Z4 gate 5: the ward's rest point is reachable from the hub's own route, and
/// it is a **detour** — the lane still crosses the zone with every cell of the
/// nook deleted. A rest you have to walk through is a corridor with a campfire
/// in it, which is exactly what a hub must not be.
///
/// Binding: 165 standable cells, 6 of them the nook, re-walked without them.
/// Teeth: `hearth/mouth_sealed` (163 standable).
#[test]
fn the_hubs_hearth_is_reachable_and_off_the_route() {
    let out = expand_at(&chapel_ward(), CHAPEL_REGION, CHAPEL_SEED);
    let cells = standable_cells(&out.model);
    let (ledge, exit) = ends(&out.model);
    let hearth = out.anchors["anchor/hearth"].pos;
    assert!(
        standable(&out.model, hearth),
        "nothing can rest at {hearth:?}"
    );
    let target: BTreeSet<[i32; 3]> = [hearth].into_iter().collect();
    assert!(
        reachable_with_fall(&out.model, &cells, &ledge, &target),
        "the hub cannot reach its own hearth"
    );

    let nook = hub_nook(&out);
    assert_eq!(nook.len(), 6, "the nook's own cells");
    let without: BTreeSet<[i32; 3]> = cells.difference(&nook).copied().collect();
    assert!(
        reachable_with_fall(&out.model, &without, &ledge, &exit),
        "delete the nook and the hub stops crossing — the rest point is on the route \
         rather than beside it"
    );

    let mut sealed = chapel_ward();
    sealed.set_param("hearth/mouth_sealed", 1).unwrap();
    let shut = expand_at(&sealed, CHAPEL_REGION, CHAPEL_SEED);
    let shut_cells = standable_cells(&shut.model);
    assert_eq!(
        shut_cells.len(),
        163,
        "the mouth's two cells, and only they, are gone"
    );
    let (shut_ledge, shut_exit) = ends(&shut.model);
    let shut_hearth: BTreeSet<[i32; 3]> = [shut.anchors["anchor/hearth"].pos].into_iter().collect();
    assert!(
        !reachable_with_fall(&shut.model, &shut_cells, &shut_ledge, &shut_hearth),
        "the nook's mouth was filled and the hearth is still reachable — the way in \
         was never the mouth"
    );
    assert!(
        reachable_with_fall(&shut.model, &shut_cells, &shut_ledge, &shut_exit),
        "sealing the nook sealed the hub, so the red above is measuring a severed \
         zone rather than an unreachable hearth"
    );
}

/// The nook's own standable cells, read off the anchor inside it and the rule's
/// published width — the same derivation `tests/staging.rs` uses at piece scale.
fn hub_nook(out: &Expansion) -> BTreeSet<[i32; 3]> {
    let hearth = out.anchors["anchor/hearth"].pos;
    standable_cells(&out.model)
        .into_iter()
        .filter(|c| {
            c[0] >= hearth[0]
                && c[0] < hearth[0] + hearth_ward::NOOK_WIDTH as i32
                && c[2] >= hearth[2] - 1
                && c[2] < hearth[2] + 2
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Z7 — the Bell Tower
// ---------------------------------------------------------------------------

/// **Gate 1, and the one this zone exists for: the tower is a route, and the
/// route CLIMBS.**
///
/// Every other zone is a chain across one floor, so "the pieces join" and "the
/// pieces are at the same height" are the same sentence. Here they are not: the
/// four upper pieces stand on a plinth this program writes, and if the plinth
/// and the flight's rise disagree by one course the zone is two rooms with a
/// step between them — which walks perfectly under `connected`'s ±1 edge and
/// would pass a route gate in silence.
///
/// So three claims, not one. The route walks (both ways — this zone has no
/// one-way hardware on its mainline, so it owes the stronger walk); the exit
/// face stands [`RISE`] blocks above the entry face, measured off the *model*
/// rather than off any anchor; and the seam is level, `anchor/stair-head` at the
/// exact height of the upper storey's own floor.
///
/// **The control** is what keeps the rise from being vacuous: `boulder_stair` in
/// the same box is flat by construction — its own module explains at length why
/// it does not climb — so a green here cannot be a flat chain wearing a stair's
/// anchors.
///
/// Binding: 2172 standable cells, 17 at the entry face, 19 at the exit face,
/// [`CLIMB`] treads, and one flat control read by the same code.
#[test]
fn the_bell_tower_is_a_route_and_the_route_climbs() {
    let out = expand_at(&bell_tower(), TOWER_REGION, TOWER_SEED);
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    assert_eq!(cells.len(), 2172, "the fixture's standable cells");
    assert_eq!((entry.len(), exit.len()), (17, 19), "the tower's end faces");

    assert!(
        connected(&cells, &entry, &exit),
        "the tower's pieces do not join into a route"
    );
    assert!(
        connected(&cells, &exit, &entry),
        "the tower cannot be walked back down"
    );

    // The rise, off the model: every cell of each face is at one height, and the
    // two heights differ by exactly what the flight climbs.
    let face_y = |face: &BTreeSet<[i32; 3]>| {
        let ys: BTreeSet<i32> = face.iter().map(|c| c[1]).collect();
        assert_eq!(ys.len(), 1, "a zone face at more than one height: {ys:?}");
        *ys.iter().next().unwrap()
    };
    let (bottom, top) = (face_y(&entry), face_y(&exit));
    assert_eq!(
        top - bottom,
        RISE,
        "the tower does not climb: entry at {bottom}, exit at {top}"
    );

    // ...and the seam is level rather than merely connected.
    let head = out.anchors["anchor/stair-head"].pos;
    assert_eq!(
        head[1], top,
        "the flight's head landing {head:?} is not the upper storey's own floor — \
         the plinth and the climb disagree, and the ±1 walk cannot see it"
    );
    assert_eq!(
        out.anchors["anchor/stair-foot"].pos[1], bottom,
        "the flight's foot landing is not level with the zone's entry"
    );

    // The control: the flat rule, the same box, the same reading code.
    let flat = expand_at(&boulder_stair(), TOWER_REGION, TOWER_SEED);
    let (flat_entry, flat_exit) = ends(&flat.model);
    assert!(!flat_entry.is_empty() && !flat_exit.is_empty());
    assert_eq!(
        face_y(&flat_exit) - face_y(&flat_entry),
        0,
        "the flat control climbed, so the rise above is not measuring a climb"
    );
}

/// ...and the climb has teeth. `flight/broken_step` raises one tread by an extra
/// course: the shaft still looks like a stair, everything below the break still
/// walks, and the head landing becomes unreachable. Nothing else about the zone
/// changes, so what goes red is the ascent and not the assembly.
///
/// The control is the half that matters: the *lower* tower still walks — the
/// entry face still reaches the foot of the break — so the red above is a broken
/// stair rather than a zone that stopped expanding.
#[test]
fn one_unclimbable_riser_severs_the_tower() {
    let mut broken = bell_tower();
    broken.set_param("flight/broken_step", 1).unwrap();
    let out = expand_at(&broken, TOWER_REGION, TOWER_SEED);
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    assert_eq!(
        (entry.len(), exit.len()),
        (17, 19),
        "the broken tower still has both faces, so the cut is the riser"
    );
    assert!(
        !connected(&cells, &entry, &exit),
        "one unclimbable riser and the tower still walks end to end — either the \
         knob does nothing after composition or there is a second way up"
    );

    // The control: the tower below the break is untouched.
    let steps = indexed(&out.anchors, "stair-step");
    assert_eq!(
        steps.len() as i64,
        CLIMB,
        "the flight still lays its treads"
    );
    let lowest: BTreeSet<[i32; 3]> = [*steps.last().expect("a tread")].into_iter().collect();
    assert!(
        connected(&cells, &entry, &lowest),
        "the entry cannot even reach the first tread, so the severance above is not \
         the break"
    );
}

/// **Gate 2 (§4 entry R).** The loft is a `rafter_hall` nineteen wide and six
/// tall — a box no piece fixture has — and the corbel form has to carry the
/// sightline there too: every perch legible from the loft's own door.
///
/// Teeth: `loft/span_beams` closes the truss across the nave, and the composed
/// loft goes blind exactly as a bare hall does. Control: the timbers are a
/// ceiling and not a wall, so the tower still walks.
///
/// Binding: 5 perches.
#[test]
fn every_loft_perch_is_visible_from_the_lofts_own_door() {
    let out = expand_at(&bell_tower(), TOWER_REGION, TOWER_SEED);
    let door = out.anchors["anchor/hall-door"].pos;
    assert!(standable(&out.model, door), "the loft door cell {door:?}");
    let perches = indexed(&out.anchors, "perch");
    assert_eq!(perches.len(), 5, "the composed loft carries its rafters");
    for perch in &perches {
        assert!(standable(&out.model, *perch), "the perch cell {perch:?}");
        if let Err(blocker) = sees(&out.model, door, *perch) {
            panic!(
                "the loft door {door:?} cannot see the perch {perch:?}: {blocker:?} is \
                 in the way — the twist ambush REMAKE §3 calls *exposed* is not"
            );
        }
    }

    let mut blinded = bell_tower();
    blinded.set_param("loft/span_beams", 1).unwrap();
    let shut = expand_at(&blinded, TOWER_REGION, TOWER_SEED);
    let shut_door = shut.anchors["anchor/hall-door"].pos;
    let blind: Vec<[i32; 3]> = indexed(&shut.anchors, "perch")
        .into_iter()
        .filter(|p| sees(&shut.model, shut_door, *p).is_err())
        .collect();
    assert_eq!(
        blind.len(),
        4,
        "the truss was closed across the nave and the door still saw every perch — \
         the gate proves nothing"
    );
    let cells = standable_cells(&shut.model);
    let (entry, exit) = ends(&shut.model);
    assert!(
        connected(&cells, &entry, &exit),
        "the closed truss severed the tower, so the blindness above measures the \
         wrong thing"
    );
}

/// **Gate 3 (§4 entry M).** The boss-door motif is the *dual of the fog gate*:
/// its job is that nobody arrives at the Bellkeeper without having crossed the
/// marker that says they are about to. That is a claim about the route, not
/// about the curtain, so it is asserted the way the keep's doorway is — cut the
/// threshold's own slice out of the graph and the ring must go unreachable.
///
/// Two controls, because a cut that severs everything proves nothing: the loft
/// side of the cut still walks to the entry face, and with the slice back the
/// ring is reachable again.
///
/// Binding: 532 cells on the ring side of the threshold, 1623 on the tower side,
/// 17 cells cut.
#[test]
fn no_route_reaches_the_bellkeeper_without_crossing_the_threshold() {
    let out = expand_at(&bell_tower(), TOWER_REGION, TOWER_SEED);
    let cells = standable_cells(&out.model);
    let (entry, _) = ends(&out.model);
    let narrate = out.anchors["anchor/threshold-narrate"].pos;
    let elite: BTreeSet<[i32; 3]> = [out.anchors["anchor/elite"].pos].into_iter().collect();

    assert!(
        connected(&cells, &entry, &elite),
        "the tower cannot reach its own boss ring"
    );

    // The doorband is one cell of the zone's length; cutting that slice is
    // cutting the motif and nothing else.
    let doorband: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| c[2] == narrate[2])
        .collect();
    assert_eq!(
        doorband.len(),
        17,
        "the threshold's own slice — the motif's full interior width: {doorband:?}"
    );
    let cut: BTreeSet<[i32; 3]> = cells.difference(&doorband).copied().collect();
    assert_eq!(cut.len(), cells.len() - 17);
    assert!(
        !connected(&cut, &entry, &elite),
        "with the boss threshold cut out the ring is still reachable — there is a \
         way to the Bellkeeper that never crosses the marker"
    );

    // The controls: the cut severed the ring and not the tower, and the ring is
    // genuinely on the far side of it.
    let ring_side: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| c[2] < narrate[2])
        .collect();
    let tower_side: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| c[2] > narrate[2])
        .collect();
    assert_eq!((ring_side.len(), tower_side.len()), (532, 1623));
    let loft_door: BTreeSet<[i32; 3]> = [out.anchors["anchor/hall-door"].pos].into_iter().collect();
    assert!(
        connected(&cut, &entry, &loft_door),
        "the cut also severed the loft from the entry, so the red above is not \
         about the threshold"
    );
}

/// **Gate 4 (§4 entry E).** The Bellkeeper's ring is open ground with a lane
/// past it on each side — REMAKE §6's "kept anti-Capra", built as geometry
/// rather than as a rule somebody has to remember. Bound inside the ring's own
/// run, exactly as Z3 binds it, because the tower's own length is a stair and a
/// loft and does not carry flank bands.
///
/// Binding: 2 routes at the default over the ring's twenty-cell run; 3 teeth
/// configurations.
#[test]
fn the_bellkeepers_ring_keeps_a_lane_on_each_side() {
    for (knob, want) in [(0, 2), (1, 1), (2, 1), (3, 0)] {
        let mut program = bell_tower();
        program.set_param("ring/seal_flank", knob).unwrap();
        let out = expand_at(&program, TOWER_REGION, TOWER_SEED);
        let elite = out.anchors["anchor/elite"].pos;
        assert_eq!(
            arena_flank_routes(&out, elite, TOWER_RING_RUN),
            want,
            "ring/seal_flank = {knob}"
        );
    }
}

/// **Gate 5 (§4 entry L).** The counterweight shaft, and the two things a lift's
/// geometry owes that no campaign JSON can assert about itself.
///
/// *It is reached through exactly one doorway.* The tower walks end to end with
/// the shaft standing (the control that separates "the shaft is a branch" from
/// "the zone is impassable"); the landing is reachable; and `tee/sealed` plugs
/// the junction's own doorway and nothing else, after which the landing is
/// unreachable while the tower still walks.
///
/// *It only drops.* The pit is reachable from the entry face under walk-and-fall
/// — the player's own model, which is how a body gets into an empty shaft — and
/// the entry face is **not** reachable from the pit under the plain ±1 step. The
/// same predicate pair `drop_shaft` and `lift_shaft` are gated on, re-bound to
/// the composed zone.
///
/// *Its landing meets the junction's.* The tee's doorway and the shaft's face
/// doorway are placed by two pieces turned 90° to each other, so they are
/// asserted to be one column rather than merely connected. That is the gate a
/// misplaced lane reds: moving `lift_shaft`'s own lane off centre (`rel(1)` →
/// `abs(1)` on its first split) makes the tower unable to reach its own landing,
/// with every other gate in the suite still green.
///
/// Binding: 1 landing sill in the strip, 1 pit, 2172 standable cells; the sealed
/// variant is 2171.
#[test]
fn the_counterweight_shaft_is_entered_once_and_only_drops() {
    let out = expand_at(&bell_tower(), TOWER_REGION, TOWER_SEED);
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    let landings = tower_shaft_landings(&out);
    let pit: BTreeSet<[i32; 3]> = [out.anchors["anchor/lift-pit"].pos].into_iter().collect();
    assert_eq!(landings.len(), 1, "the shaft's landings: {landings:?}");

    assert!(
        connected(&cells, &entry, &exit),
        "the shaft sealed the tower — a branch that severs the zone it hangs off is \
         not a branch"
    );
    assert!(
        connected(&cells, &entry, &landings),
        "the tower cannot reach its own lift landing"
    );

    // ...and the two doorways are one column, which is the claim the walk above
    // would only tell us about indirectly. The tee's doorway and the shaft's
    // face doorway are placed by two pieces turned 90° to each other, so this is
    // the seam a reader would most expect to be off by one.
    let door = out.anchors["anchor/branch-door"].pos;
    let sill = *landings.iter().next().expect("a landing");
    assert_eq!(
        (sill[1], sill[2], door[0] - sill[0]),
        (door[1], door[2], 1),
        "the junction's doorway {door:?} and the shaft's landing {sill:?} are not \
         the same column — the two pieces' centres disagree"
    );
    assert!(
        reachable_with_fall(&out.model, &cells, &entry, &pit),
        "a body cannot get into the shaft at all, so its one-wayness is vacuous"
    );
    assert!(
        !connected(&cells, &pit, &entry),
        "the shaft's pit walks back into the tower — the hub-opener is a staircase"
    );
    // ...and the negative is not merely the pit being sealed off from itself:
    // the shaft's own landing is what it fails to reach.
    assert!(
        !connected(&cells, &pit, &landings),
        "the pit walks back up to the landing it fell from"
    );

    // The teeth, and the control beside them.
    let mut sealed = bell_tower();
    sealed.set_param("tee/sealed", 1).unwrap();
    let shut = expand_at(&sealed, TOWER_REGION, TOWER_SEED);
    let shut_cells = standable_cells(&shut.model);
    let (shut_entry, shut_exit) = ends(&shut.model);
    assert_eq!(
        shut_cells.len(),
        cells.len() - 1,
        "the junction's doorway cell, and only it, is gone"
    );
    assert!(
        !connected(&shut_cells, &shut_entry, &tower_shaft_landings(&shut)),
        "the junction's doorway was filled and the shaft is still reachable — the \
         way in was never the doorway"
    );
    assert!(
        connected(&shut_cells, &shut_entry, &shut_exit),
        "sealing the junction sealed the tower, so the red above measures an \
         impassable zone rather than an unreachable shaft"
    );
}

/// **Gate 6.** The tower's one piece of seam arithmetic is *checked*, not hoped.
///
/// The plinth is cut to a declared number and the flight's rise is derived from
/// the box; the plan guards that they agree, and that the shaft's lowest station
/// is that same number. Dial any of the three and the expansion is a refusal
/// naming the rule — never a tower with a step at its own landing or a lift
/// landing that opens into solid mass.
///
/// Binding: 4 drifts, each shown refusing, plus the control that the untouched
/// program expands.
#[test]
fn the_towers_plinth_arithmetic_is_guarded_not_hoped() {
    expand_at(&bell_tower(), TOWER_REGION, TOWER_SEED);
    assert_eq!(SILL, CLIMB, "the shaft's sill is the flight's own climb");

    for (knob, value) in [
        // the station moves off the upper storey's floor
        ("shaft/sill", SILL + 1),
        // the flight climbs faster, so the plinth is now too thin
        ("flight/tread", 1),
        // ...and slower, so it is too thick
        ("flight/landing_run", 4),
        // a longer landing run for the ring shortens the flight's own
        ("ring_run", 22),
    ] {
        let mut drifted = bell_tower();
        drifted.set_param(knob, value).unwrap();
        let err = expand(&drifted, TOWER_REGION, &ExpandOptions::seeded(TOWER_SEED)).unwrap_err();
        assert!(
            err.to_string().contains("no alternative of rule"),
            "expected {knob} = {value} to be refused, got: {err}"
        );
    }
}

// ---------------------------------------------------------------------------
// What composition itself has to get right
// ---------------------------------------------------------------------------

/// The mainline pieces stand in travel order along the zone's own axis, and
/// every anchor they declare faces down it. This is the cheap, total check that
/// no piece was turned: a rotated piece declares its anchors facing `west`, and
/// its wall across the route.
///
/// **Two zones now turn a piece on purpose**, and the distinction is the whole
/// value of this gate, so it is stated per zone rather than allowed for by name.
/// A `west` facing is correct for exactly three kinds of anchor:
///
/// * one the vocabulary aims across the route through a `reorient` — the cliff
///   recess, the ambush alcove, the storeroom tell, the broken grate, and the
///   tee's own branch doorway;
/// * one declared by a piece the *zone* laid across the mainline, which is what
///   a branch is: `far_side_bar`'s `gate`/`unlock` in Z4 and Z6.
///
/// Everything else faces `north`, down travel. A piece that turned by accident
/// still reds this gate, because its zone's set does not name it.
///
/// Binding: 84 anchors — 1 (Z0), 8 (Z1: 4 niches, 4 watch cells), 16 (Z2), 6
/// (Z3), 6 (Z4), 14 (Z5), 9 (Z6), 24 (Z7: 9 treads, 5 perches and the rest). Of
/// those, exactly 26 face across, and that count is pinned too: 4 cliff
/// recesses, the two ambush alcoves, the storeroom tell, the broken grate, the
/// boulder stair's two safe pockets, the 9 branch anchors of Z2, Z3, Z4 and Z6,
/// and Z7's 4.
#[test]
fn the_pieces_stand_in_travel_order() {
    // Z2 is now the long one: six pieces on the mainline, and the order is the
    // beat sequence — read the portcullis, cross it, meet the corner ambush, jam
    // the release from the head, run the tread, pass the sally port, cross the
    // boss threshold, spill out.
    let ward = expand_at(&gate_ward(), WARD_REGION, WARD_SEED);
    let z = |out: &Expansion, name: &str| out.anchors[name].pos[2];
    let order = [
        "anchor/landing",
        "anchor/spill",
        "anchor/threshold-narrate",
        "anchor/stair-run",
        "anchor/run-head",
        "anchor/release",
        "anchor/alcove",
        "anchor/threshold",
        "anchor/gate",
        "anchor/watch",
    ];
    for pair in order.windows(2) {
        assert!(
            z(&ward, pair[0]) < z(&ward, pair[1]),
            "the gatehouse's pieces are out of travel order at {pair:?}: {:#?}",
            ward.anchors
        );
    }

    let keep = expand_at(&hall_keep(), KEEP_REGION, KEEP_SEED);
    let order = [
        "anchor/landing",
        "anchor/hatch",
        "anchor/threshold-narrate",
        "anchor/bait",
        "anchor/tell",
        "anchor/store-line",
        "anchor/alcove",
        "anchor/threshold",
        "anchor/hall-door",
    ];
    for pair in order.windows(2) {
        assert!(
            z(&keep, pair[0]) < z(&keep, pair[1]),
            "the keep's pieces are out of travel order at {pair:?}: {:#?}",
            keep.anchors
        );
    }
    // The lure and the body over it are one column, so travel order does not
    // separate them — what does is that the watcher is *above*.
    assert_eq!(
        keep.anchors["anchor/bait"].pos[2],
        keep.anchors["anchor/bait-perch"].pos[2]
    );

    // Z4: the chute's landing and hatch, and the junction's doorway between them
    // and the way out. The two shortcut anchors are deliberately *not* in this
    // chain — they are off the mainline, and the assertion under it is that they
    // sit inside the junction's own run rather than anywhere along the route.
    let chapel = expand_at(&chapel_ward(), CHAPEL_REGION, CHAPEL_SEED);
    let order = [
        "anchor/branch-door",
        "anchor/hearth",
        "anchor/landing",
        "anchor/hatch",
    ];
    for pair in order.windows(2) {
        assert!(
            z(&chapel, pair[0]) < z(&chapel, pair[1]),
            "the hub's pieces are out of travel order at {pair:?}: {:#?}",
            chapel.anchors
        );
    }

    // Z3: the crossing, the junction, and the lower ward's own fight. The
    // keeper's anchor sits on the gatehouse at the far end of the crossing, so
    // it comes last; the two shortcut anchors are off the mainline and are
    // checked below with Z4's and Z6's.
    let drowned = expand_at(&drowned_ward(), DROWNED_REGION, DROWNED_SEED);
    let order = [
        "anchor/elite",
        "anchor/branch-door",
        "anchor/keeper-elite",
        "anchor/causeway-head",
    ];
    for pair in order.windows(2) {
        assert!(
            z(&drowned, pair[0]) < z(&drowned, pair[1]),
            "the drowned ward's pieces are out of travel order at {pair:?}: {:#?}",
            drowned.anchors
        );
    }

    // Z6 is the long one: five pieces, and the order is also the beat sequence —
    // spill in, read the volley, cross it, find the grate, pass the sally port,
    // meet the elite.
    let deep = expand_at(&cistern_deep(), DEEP_REGION, DEEP_SEED);
    let order = [
        "anchor/elite",
        "anchor/branch-door",
        "anchor/grate-secret",
        "anchor/gate",
        "anchor/watch",
        "anchor/landing",
        "anchor/spill",
    ];
    for pair in order.windows(2) {
        assert!(
            z(&deep, pair[0]) < z(&deep, pair[1]),
            "the cistern's pieces are out of travel order at {pair:?}: {:#?}",
            deep.anchors
        );
    }

    // Z7 is the tall one, and the only zone whose travel order is also a
    // *climb*: the ring and the boss door stand a storey above the flight the
    // player comes up, and the anchors still run in one order down the zone's
    // own axis. The lift's three anchors are off the mainline and are checked
    // below with the other branches'.
    let tower = expand_at(&bell_tower(), TOWER_REGION, TOWER_SEED);
    let order = [
        "anchor/elite",
        "anchor/threshold-narrate",
        "anchor/branch-door",
        "anchor/hall-door",
        "anchor/stair-head",
        "anchor/stair-foot",
    ];
    for pair in order.windows(2) {
        assert!(
            z(&tower, pair[0]) < z(&tower, pair[1]),
            "the bell tower's pieces are out of travel order at {pair:?}: {:#?}",
            tower.anchors
        );
    }

    // The branch's own anchors are off the mainline, so travel order does not
    // apply to them — what does is that they lie *beside* the junction that opens
    // onto them and not beside some other piece.
    for (out, doorway, gate) in [
        (&chapel, "anchor/branch-door", "anchor/gate"),
        (&deep, "anchor/branch-door", "anchor/sally-gate"),
        (&drowned, "anchor/branch-door", "anchor/gate"),
        (&ward, "anchor/branch-door", "anchor/sally-gate"),
    ] {
        for name in [gate, "anchor/unlock"] {
            assert!(
                (z(out, name) - z(out, doorway)).abs() <= 2,
                "{name} is not beside the doorway that opens onto it: {:#?}",
                out.anchors
            );
        }
    }
    // Z7's branch is the shaft, and its three anchors owe the same: the station
    // and the pit are one column, and the call control is beside the doorway
    // that reaches them.
    for name in [
        "anchor/lift-station-1",
        "anchor/lift-call-1",
        "anchor/lift-pit",
    ] {
        assert!(
            (z(&tower, name) - z(&tower, "anchor/branch-door")).abs() <= 2,
            "{name} is not beside the doorway that opens onto it: {:#?}",
            tower.anchors
        );
    }

    // Every anchor faces the way its rule — or its zone — meant it to. See this
    // test's own note for the two ways an anchor earns a `west`.
    let shore = expand_at(&barrow_shore(), SHORE_REGION, SHORE_SEED);
    let road = expand_at(&cliff_road(), CLIFF_REGION, CLIFF_SEED);
    let branch: &[&str] = &["anchor/branch-door", "anchor/gate", "anchor/unlock"];
    let sally: &[&str] = &["anchor/branch-door", "anchor/sally-gate", "anchor/unlock"];
    // Z7's turned set: the tee's doorway, plus every anchor of the shaft the
    // zone laid across the mainline — the same reason Z4's and Z6's bars earn a
    // `west`, one piece further along.
    let lift: &[&str] = &[
        "anchor/branch-door",
        "anchor/lift-station-1",
        "anchor/lift-call-1",
        "anchor/lift-pit",
    ];
    let mut checked = 0;
    let mut across_seen = 0;
    for (out, turned) in [
        (&shore, &[][..]),
        (&road, &[][..]),
        (&ward, sally),
        (&chapel, branch),
        (&keep, &[][..]),
        (&deep, sally),
        (&drowned, branch),
        (&tower, lift),
    ] {
        for (name, anchor) in &out.anchors {
            let across = name == "anchor/alcove"
                || name == "anchor/tell"
                || name == "anchor/grate-secret"
                || name.starts_with("anchor/pocket-")
                || (name.starts_with("anchor/niche-") && !name.starts_with("anchor/niche-watch-"))
                || turned.contains(&name.as_str());
            let want = if across { "west" } else { "north" };
            assert_eq!(
                anchor.facing.as_str(),
                want,
                "{name} faces the wrong way — the piece that declared it was turned"
            );
            checked += 1;
            across_seen += usize::from(across);
        }
    }
    assert_eq!(checked, 84, "the gate checked {checked} anchors");
    assert_eq!(across_seen, 26, "the gate allowed {across_seen} across");
}

/// The frame constraint, enforced as a refusal: a piece run shorter than the
/// zone is wide has no applicable alternative.
///
/// Teeth are the other half of this test. A refusal only means something if the
/// thing refused would really have gone wrong — so the same short box is handed
/// to the piece on its own, and it builds a threshold turned 90°: an anchor
/// facing `west` instead of `north`, i.e. a wall across the route rather than
/// along it. That is the defect the zone guard exists to make impossible.
#[test]
fn a_piece_run_shorter_than_the_zone_is_refused_not_turned() {
    let mut squat = hall_keep();
    squat.set_param("door_run", 7).unwrap();
    let err = expand(&squat, KEEP_REGION, &ExpandOptions::seeded(KEEP_SEED)).unwrap_err();
    assert!(
        err.to_string().contains("no alternative of rule"),
        "expected a refusal, got: {err}"
    );

    // The control: the refused shape is a real defect, not a cautious guard.
    let wide = Box3::at_origin([KEEP_REGION.size[0], KEEP_REGION.size[1], 7]);
    let turned = expand_at(
        &delvewright_grammar::library::ambush_door(),
        wide,
        KEEP_SEED,
    );
    assert_eq!(
        turned.anchors["anchor/threshold"].facing.as_str(),
        "west",
        "a box wider than it is long did not turn the threshold, so the guard is \
         guarding against nothing"
    );

    // The same guard, on a zone four times as wide — where the run a piece needs
    // is correspondingly longer, and the defect it prevents is the same one.
    let mut stubby = cistern_deep();
    stubby.set_param("gallery_run", 7).unwrap();
    let err = expand(&stubby, DEEP_REGION, &ExpandOptions::seeded(DEEP_SEED)).unwrap_err();
    assert!(
        err.to_string().contains("no alternative of rule"),
        "expected a refusal, got: {err}"
    );
    let stub_box = Box3::at_origin([DEEP_REGION.size[0], DEEP_REGION.size[1], 7]);
    let turned_bay = expand_at(&watch_bay(), stub_box, DEEP_SEED);
    assert_eq!(
        turned_bay.anchors["anchor/watch"].facing.as_str(),
        "west",
        "a box wider than it is long did not turn the watch bay, so the guard is \
         guarding against nothing"
    );

    // The same guard on the tallest zone, and here it is isolated on purpose:
    // Z7's plan also guards the climb arithmetic, so shortening a run outright
    // would be refused for two reasons at once and this test would not know
    // which. Moving a cell from the boss door's run to the ring's leaves the
    // upper storey — and therefore the flight's run and the plinth — untouched,
    // so the *only* clause that can fail is the frame one.
    let mut squat_tower = bell_tower();
    squat_tower.set_param("door_run", 19).unwrap();
    squat_tower.set_param("ring_run", 21).unwrap();
    let err = expand(
        &squat_tower,
        TOWER_REGION,
        &ExpandOptions::seeded(TOWER_SEED),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("no alternative of rule"),
        "expected a refusal, got: {err}"
    );
    let flat_box = Box3::at_origin([TOWER_REGION.size[0], 6, 19]);
    let turned_motif = expand_at(
        &delvewright_grammar::library::threshold_motif(),
        flat_box,
        TOWER_SEED,
    );
    assert_eq!(
        turned_motif.anchors["anchor/threshold-narrate"]
            .facing
            .as_str(),
        "west",
        "a box wider than it is long did not turn the boss threshold, so the guard \
         is guarding against nothing"
    );
}

// ---------------------------------------------------------------------------
// The seam's limits — what composition can now do, and what it still cannot
// ---------------------------------------------------------------------------

/// Two pieces laid end to end in one throwaway program: the smallest thing that
/// is a composition at all, and the fixture the claims below are read off.
fn chained(parts: [(&str, &Program); 2]) -> Program {
    let none = AnchorRenames::new();
    chained_renaming([
        (parts[0].0, parts[0].1, &none),
        (parts[1].0, parts[1].1, &none),
    ])
}

/// The same chain, with each piece given the anchor renames it is included
/// under.
fn chained_renaming(parts: [(&str, &Program, &AnchorRenames<'_>); 2]) -> Program {
    let plan = Program::new("chain", "plan").rule_alts(
        "plan",
        vec![Alternative::new(Node::Split(Split {
            axis: Axis::Z,
            sizes: vec![Size::rel(1), Size::rel(1)],
            rounding: Default::default(),
            repeat: false,
            orient: Default::default(),
            children: parts
                .iter()
                .map(|(prefix, source, _)| Node::call(&entry(prefix, source)))
                .collect(),
        }))],
    );
    parts.iter().fold(plan, |acc, (prefix, source, renames)| {
        include_renaming(acc, source, prefix, renames).unwrap_or_else(|e| panic!("{prefix}: {e}"))
    })
}

/// The default, stated as a test rather than as a comment: an include does not
/// rename anchors *on its own*, because an anchor name is the campaign's
/// contract. So a zone that includes the same piece twice and says nothing gets
/// two declarations of one name, and expansion refuses loudly instead of letting
/// the second quietly win.
///
/// The refusal now carries its own remedy, which is the difference between a
/// wall and a signpost: the message names `include_renaming`, so the next reader
/// meets the fix rather than the limit.
#[test]
fn including_one_piece_twice_without_saying_so_is_a_loud_anchor_collision() {
    let bay = watch_bay();
    let twice = chained([("first", &bay), ("second", &bay)]);
    twice.validate().expect("both copies resolve");

    let err = expand(
        &twice,
        Box3::at_origin([11, 7, 40]),
        &ExpandOptions::seeded(1),
    )
    .unwrap_err();
    let text = err.to_string();
    assert!(
        text.contains("declared twice") && text.contains("first/") && text.contains("second/"),
        "expected an anchor collision naming both copies' rules, got: {text}"
    );
    assert!(
        text.contains("include_renaming"),
        "the collision does not name its remedy, so it reads as a dead end: {text}"
    );
}

/// **Seam limit 1, closed.** It never took two copies of one piece to hit that
/// collision: two *different* pieces that happen to declare the same anchor name
/// collided identically, and two pairs in today's vocabulary do — `causeway` and
/// `elite_ground` both declare `anchor/elite`, `watch_bay` and `far_side_bar`
/// both declare `anchor/gate`. That refused Z3's **T** + **E** and Z6's **F**,
/// not for want of a shape but for want of a name.
///
/// A rename at the include site is the name. It is deliberately *explicit* and
/// per-anchor rather than a blanket prefix: a ward with a causeway keeper and a
/// dormant elite has two genuinely different elites and the campaign has to be
/// able to tell them apart, so the zone writes down which is which. A prefix
/// would have silently moved every anchor a `timed-gate` already binds.
///
/// Binding: 2 pairs, 4 anchors, each expanded and each read back by name and by
/// declaring rule. Control: the same two pairs with no rename still collide
/// (the test above, and
/// `two_pieces_that_share_a_stem_still_collide_when_nobody_renames`).
#[test]
fn a_rename_at_the_include_site_lets_two_pieces_declare_one_stem() {
    const WARD: Box3 = Box3::at_origin([21, 12, 60]);
    let cases = [
        (
            "elite",
            "keeper-elite",
            causeway(),
            elite_ground(),
            "near/post_column",
            "far/elite_column",
        ),
        (
            "gate",
            "shortcut-gate",
            far_side_bar(),
            watch_bay(),
            "near/door_column",
            "far/span_column",
        ),
    ];
    let mut checked = 0;
    for (stem, renamed, near, far, near_rule, far_rule) in &cases {
        let renames = AnchorRenames::from([(*stem, *renamed)]);
        let none = AnchorRenames::new();
        let program = chained_renaming([("near", near, &renames), ("far", far, &none)]);
        program.validate().expect("both pieces resolve");
        let out = expand(&program, WARD, &ExpandOptions::seeded(1))
            .unwrap_or_else(|e| panic!("{} + {}: {e}", near.name, far.name));

        let moved = out
            .anchors
            .get(&format!("anchor/{renamed}"))
            .unwrap_or_else(|| panic!("the renamed anchor is missing: {:#?}", out.anchors));
        let kept = out
            .anchors
            .get(&format!("anchor/{stem}"))
            .unwrap_or_else(|| panic!("the unrenamed anchor is missing: {:#?}", out.anchors));
        assert_eq!(
            (moved.declared_by.as_str(), kept.declared_by.as_str()),
            (*near_rule, *far_rule),
            "the two anchors came from the wrong pieces"
        );
        assert_ne!(moved.pos, kept.pos, "two names, one place");
        checked += 2;
    }
    assert_eq!(checked, 4, "the gate read back {checked} anchors");
}

/// ...and the rename is opt-in, which is the whole safety property: the same two
/// pairs with nothing said still collide, so no existing anchor contract moved
/// when this landed.
///
/// Binding: 2 pairs, each refused by name.
#[test]
fn two_pieces_that_share_a_stem_still_collide_when_nobody_renames() {
    let pairs = [
        ("anchor/elite", causeway(), elite_ground()),
        ("anchor/gate", watch_bay(), far_side_bar()),
    ];
    for (anchor, near, far) in &pairs {
        let program = chained([("near", near), ("far", far)]);
        program.validate().expect("both pieces resolve");
        let err = expand(
            &program,
            Box3::at_origin([21, 12, 60]),
            &ExpandOptions::seeded(1),
        )
        .unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains(anchor) && text.contains("declared twice"),
            "{} + {} should have collided on {anchor}, got: {text}",
            near.name,
            far.name
        );
    }
}

/// **Seam limit 2, closed.** `causeway` used to be a **terminus** with no way
/// to stop being one, and that was the whole of what Z3 waited on: its far face
/// carried no standable cell at berm height, so a zone that put anything past it
/// handed the player a wall, and a zone that put it last ended at one. No
/// orientation helped — a grammar orientation is a permutation without
/// reflection, so the post cannot be turned to the entry end either.
///
/// The rule now exposes `berm_gate`, and this test is what keeps both halves
/// honest. Terminus is still the **default**, because a guard post you can
/// simply walk under is a weaker piece and nobody should get one by accident;
/// the piece is deliberately unreachable from the berm either way ("not a
/// landing", the same move that keeps `rafter_hall`'s perches off the nave). So
/// the assertions below are unchanged from the ones that recorded the finding —
/// they are now the proof that the default did not move — and the last one is
/// the finding's closure. Z3 is [`drowned_ward`], above.
///
/// Binding: the 2 `Z`-slices of the guard station, and the 22-cell berm.
/// Control: the berm itself is still a route across the ward, so what is severed
/// with the gate shut is the exit and not the crossing.
#[test]
fn the_causeway_is_a_terminus_until_its_berm_gate_is_opened() {
    const WARD: Box3 = Box3::at_origin([9, 10, 24]);
    let out = expand_at(&causeway(), WARD, 1);
    let cells = standable_cells(&out.model);
    let berm_top = out.anchors["anchor/causeway-head"].pos[1];
    let post = out.anchors["anchor/elite"].pos;
    assert!(post[1] > berm_top, "the post is the elevated one");

    // The zone-facing face of the piece: at the far end there is nowhere to
    // stand at berm height, and the cantilever slice has no floor at all.
    let at_z = |z: i32| -> Vec<[i32; 3]> { cells.iter().copied().filter(|c| c[2] == z).collect() };
    assert!(
        at_z(1).is_empty(),
        "the cantilever slice has a floor: {:?}",
        at_z(1)
    );
    assert!(
        at_z(0).iter().all(|c| c[1] == post[1]),
        "something stands at the far face below the post: {:?}",
        at_z(0)
    );

    // ...and the post is not reachable from the berm, so the piece cannot even
    // be crossed, let alone continued past.
    let berm: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[1] == berm_top).collect();
    assert_eq!(berm.len(), 22, "the berm");
    let (entry, exit) = ends(&out.model);
    assert!(
        !reachable_with_fall(&out.model, &cells, &entry, &exit),
        "the causeway reaches its own far face — this finding is stale and Z3 can be \
         chained after all"
    );

    // The control: the crossing itself works. What is missing is only the exit.
    let head: BTreeSet<[i32; 3]> = berm.iter().copied().filter(|c| c[2] == 23).collect();
    let toe: BTreeSet<[i32; 3]> = berm.iter().copied().filter(|c| c[2] == 2).collect();
    assert!(
        connected(&berm, &head, &toe),
        "the berm does not even cross the ward, so the red above is a broken fixture"
    );

    // ...and the closure: the same box, the same seed, one knob.
    let mut open = causeway();
    open.set_param("berm_gate", 1).unwrap();
    let out = expand_at(&open, WARD, 1);
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    assert!(
        connected(&cells, &entry, &exit),
        "`berm_gate` is open and the piece is still a terminus"
    );
}

/// The branch fixture's box: a mainline three wide with a seven-deep strip
/// alongside it, long enough for a real chain either side of the junction.
const BRANCH_REGION: Box3 = Box3::at_origin([10, 6, 24]);
/// How deep the branch strip runs off the mainline.
///
/// **Strictly greater than [`TEE_RUN`]**, and that is the frame constraint §5c
/// already documents rather than a magic number: every §5b rule opens with
/// `z(Largest)`, so `far_side_bar` turns its travel onto the longer horizontal
/// axis of whatever box it is handed. A branch box deeper than it is wide puts
/// that axis *across* the mainline, which aims the bar's near room at the tee's
/// doorway. A wider-than-deep box would build the bar along the chain instead
/// — the piece turned, which `the_pieces_stand_in_travel_order` is the general
/// form of.
const BRANCH_DEPTH: i64 = 7;
/// The tee's own run along the chain, and the branch box's width — the same
/// number on purpose, so the doorway the tee cuts at the middle of its run lands
/// inside the branch's near room rather than in one of its end walls.
const TEE_RUN: i64 = 5;
/// Neither piece of the branch draws from the seed; the grate's break does, and
/// the gates below do not read it.
const BRANCH_SEED: u64 = 1;

/// A chain with a branch: `tee_passage` + `broken_grate` down the mainline, and
/// a `far_side_bar` in a side strip beside the tee's doorway.
///
/// The zone writes no encounter geometry — only the two splits that carve the
/// strip out of the box and the plain fill that walls the strip's margins, which
/// is the same "mass no piece can know about" `cliff_road`'s gulf is. Everything
/// else is `include` and `call`.
fn branch_chain() -> Program {
    fn cut(axis: Axis, sizes: Vec<Size>, children: Vec<Node>) -> Node {
        Node::Split(Split {
            axis,
            sizes,
            // `Rounding::Start`, not the default truncation: a truncated
            // relative piece leaves the far end of the box unwritten, and an
            // unwritten cell is air — a hole in the strip's own margin wall.
            rounding: delvewright_grammar::ir::Rounding::Start,
            repeat: false,
            orient: Default::default(),
            children,
        })
    }
    let tee = delvewright_grammar::library::tee_passage();
    let bar = far_side_bar();
    let vent = broken_grate();
    let plan = Program::new("branch_chain", "plan")
        .role("margin", BlockState::simple("deepslate"))
        .rule(
            "plan",
            cut(
                Axis::X,
                vec![Size::abs(BRANCH_DEPTH), Size::rel(1)],
                vec![Node::call("strip"), Node::call("mainline")],
            ),
        )
        // The strip: the branch's own box at the tee's own run, solid rock for
        // the rest of its length. The fill is inert mass, not a gate — what
        // makes the tee's doorway the only way into the branch is that
        // `far_side_bar` walls its own two side faces, which is the same §5b
        // property this seam limit came from. (Measured: replacing the fill with
        // air reds nothing, because a floorless strip has no standable cell in
        // it either way.)
        .rule(
            "strip",
            cut(
                Axis::Z,
                vec![Size::abs(TEE_RUN), Size::rel(1)],
                vec![Node::call(&entry("bar", &bar)), Node::fill("margin")],
            ),
        )
        .rule(
            "mainline",
            cut(
                Axis::Z,
                vec![Size::abs(TEE_RUN), Size::rel(1)],
                vec![
                    Node::call(&entry("tee", &tee)),
                    Node::call(&entry("vent", &vent)),
                ],
            ),
        );
    let none = AnchorRenames::new();
    [("tee", &tee), ("bar", &bar), ("vent", &vent)]
        .into_iter()
        .fold(plan, |acc, (prefix, source)| {
            include_renaming(acc, source, prefix, &none).unwrap_or_else(|e| panic!("{prefix}: {e}"))
        })
}

/// **The obligation this round exists for, discharged at the seam.** Every gate
/// in `tests/staging.rs` judges the flight in a bare box; what a zone needs is
/// that the climb survives being *composed* — that a piece run can carry a
/// player in at ground level and out at the top.
///
/// This is the smallest thing that is a composition at all: `tee_passage` as
/// the approach, `stair_flight` beyond it, the same throwaway `chained` fixture
/// seam limit 3 is read off. Nothing new is needed at the seam — a flight's
/// foot landing sits at the same floor course every flat piece in the
/// vocabulary uses, so the two mate the way any two chain pieces do.
///
/// Binding: 96 standable cells, 6 of them the zone's entry face, and a rise of
/// 7 between that face and `anchor/stair-head`. Control: the same walk from the
/// entry face to the flight's *foot* landing, which is level — so a green here
/// cannot be a flat corridor wearing a stair's anchors.
#[test]
fn a_zone_can_compose_a_route_a_player_walks_up() {
    /// Two 22-long pieces: the approach, then the flight.
    const TOWER: Box3 = Box3::at_origin([5, 14, 44]);
    let flight = stair_flight();
    let approach = delvewright_grammar::library::tee_passage();
    // `chained` puts its first part at local `Z`-min — the travel destination —
    // so the flight is named first and the approach second.
    let zone = chained([("flight", &flight), ("approach", &approach)]);
    zone.validate().expect("the composed zone resolves");

    let out = expand_at(&zone, TOWER, 1);
    let model = &out.model;
    let cells = standable_cells(model);
    assert_eq!(cells.len(), 133, "the fixture's standable cells");
    let (entry, _) = ends(model);
    assert_eq!(entry.len(), 3, "the zone's entry face");

    let head = out.anchors["anchor/stair-head"].pos;
    let foot = out.anchors["anchor/stair-foot"].pos;
    let entry_y = entry.iter().next().expect("an entry cell")[1];
    assert_eq!(
        foot[1], entry_y,
        "the flight's foot landing is not level with the zone's entry, so the \
         pieces did not mate"
    );
    assert_eq!(
        head[1] - entry_y,
        7,
        "the composed route does not climb: entry at {entry_y}, head at {head:?}"
    );
    assert!(
        connected(&cells, &entry, &[head].into_iter().collect()),
        "a player entering the zone cannot reach the top of the flight"
    );
    assert!(
        connected(&cells, &[head].into_iter().collect(), &entry),
        "and cannot walk back down"
    );
}

/// **Seam limit 3, closed — but the first half of this test is still true, and
/// that is the point.** A `far_side_bar` laid *in* the chain seals the zone's
/// own route rather than sitting beside it, which is the opposite of what a
/// shortcut is (spec-0016 §2: a short way back between two rest points, earned
/// from the far side). Every §5b rule walls its own two side faces, so a
/// composition is a chain along one axis and a zone had no way to hand a piece a
/// box *off* the route.
///
/// [`tee_passage`] is that way, and it is vocabulary rather than a new
/// primitive: the IR already expressed "a chain segment whose one side face
/// carries a doorway" — `ambush_door` and `far_side_bar` are exactly that
/// construction, merely turned 90° from where a branch needs it. So the second
/// half of this test lays the *same bar* beside the route instead of on it, and
/// asserts the three things that make it a shortcut rather than a wall.
///
/// Binding, first half: the chain's standable cells, walked twice; `bar/unbarred`
/// opens the same doorway and the same walk connects, so what sealed the chain
/// was the bar and not a seam that never joined.
///
/// Binding, second half: 43 standable cells (25 on the mainline, 18 in the
/// branch), 9 of them the branch's near room, 1 the doorway column that is cut
/// and re-walked.
#[test]
fn a_barred_door_on_the_route_seals_the_chain() {
    const CHAIN: Box3 = Box3::at_origin([5, 6, 24]);
    let vent = broken_grate();
    let bar = far_side_bar();
    let program = chained([("bar", &bar), ("vent", &vent)]);

    let out = expand_at(&program, CHAIN, 1);
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    assert!(!entry.is_empty() && !exit.is_empty(), "the chain has ends");
    assert!(
        !reachable_with_fall(&out.model, &cells, &entry, &exit),
        "a barred door on the route did not seal the chain — either this finding is \
         stale or the fixture never put the bar on the route"
    );

    let mut open = program;
    open.set_param("bar/unbarred", 1).unwrap();
    let opened = expand_at(&open, CHAIN, 1);
    let open_cells = standable_cells(&opened.model);
    let (open_entry, open_exit) = ends(&opened.model);
    assert!(
        connected(&open_cells, &open_entry, &open_exit),
        "drawing the bar did not open the chain, so the seal above proves nothing \
         about the bar"
    );

    // ------------------------------------------------------------------
    // ...and the same bar, laid *beside* the route through a `tee_passage`.
    // ------------------------------------------------------------------
    let branch = branch_chain();
    let out = expand_at(&branch, BRANCH_REGION, BRANCH_SEED);
    let cells = standable_cells(&out.model);
    assert_eq!(cells.len(), 43, "the composed chain's standable cells");
    let unlock: BTreeSet<[i32; 3]> = [out.anchors["anchor/unlock"].pos].into_iter().collect();
    let door = out.anchors["anchor/branch-door"].pos;

    // 1. The CONTROL, and it is what separates "the shortcut is sealed" from
    //    "the zone is impassable": the mainline walks end to end **with the bar
    //    standing**. This is the assertion the first half of this test fails.
    let (entry, exit) = ends(&out.model);
    assert_eq!(
        (entry.len(), exit.len()),
        (1, 1),
        "the mainline's ends: {entry:?} / {exit:?}"
    );
    assert!(
        connected(&cells, &entry, &exit),
        "the branch sealed the mainline — a tee that severs the chain it is a \
         segment of is a `far_side_bar` wearing a different name"
    );

    // 2. The near side reaches the branch's near room, and stops there.
    let near_room = branch_near_room(&out, BRANCH_DEPTH as i32);
    assert_eq!(near_room.len(), 9, "the branch's near room: {near_room:?}");
    assert!(
        connected(&cells, &entry, &near_room),
        "the mainline cannot reach the branch at all — the tee's doorway opens onto \
         nothing, so the shortcut is not even a room"
    );
    assert!(
        !connected(&cells, &entry, &unlock),
        "the mainline reaches {unlock:?} while the bar stands — the shortcut has no \
         far side to earn"
    );

    // 3. Drawing the bar connects the two, and through exactly the tee's doorway:
    //    cut that one column and the branch is unreachable again.
    let mut open = branch_chain();
    open.set_param("bar/unbarred", 1).unwrap();
    let opened = expand_at(&open, BRANCH_REGION, BRANCH_SEED);
    let open_cells = standable_cells(&opened.model);
    let (open_entry, _) = ends(&opened.model);
    assert!(
        connected(&open_cells, &open_entry, &unlock),
        "drawing the bar did not open the branch, so the seal above proves nothing"
    );
    let cut: BTreeSet<[i32; 3]> = open_cells
        .iter()
        .copied()
        .filter(|c| c[0] != door[0] || c[2] != door[2])
        .collect();
    assert_eq!(
        cut.len(),
        open_cells.len() - 1,
        "exactly the doorway was cut"
    );
    assert!(
        !connected(&cut, &open_entry, &unlock),
        "with the tee's doorway plugged the unbarred branch is still reachable — the \
         way in is a seam that never closed, not the doorway this rule declares"
    );
}

/// ...and the branch has teeth. `tee/sealed` fills the tee's doorway and nothing
/// else: the branch must go unreachable **while the mainline still walks**, which
/// is what proves the doorway — and not a seam accident anywhere along the strip
/// — is the way in.
///
/// Binding: 42 standable cells (one fewer than the open fixture: the doorway),
/// 9 of them the branch's near room.
#[test]
fn sealing_the_tee_makes_the_branch_unreachable() {
    let mut sealed = branch_chain();
    sealed.set_param("tee/sealed", 1).unwrap();
    let out = expand_at(&sealed, BRANCH_REGION, BRANCH_SEED);
    let cells = standable_cells(&out.model);
    assert_eq!(cells.len(), 42, "the doorway cell, and only it, is gone");
    let (entry, exit) = ends(&out.model);

    let near_room = branch_near_room(&out, BRANCH_DEPTH as i32);
    assert_eq!(near_room.len(), 9, "the branch is still a room");
    assert!(
        !connected(&cells, &entry, &near_room),
        "the tee's doorway was filled and the branch is still reachable — the way in \
         was never the doorway, so the gate above proves nothing"
    );

    // The control: a filled doorway, not a filled zone.
    assert!(
        connected(&cells, &entry, &exit),
        "sealing the branch door sealed the mainline, so the red above is measuring \
         an impassable chain rather than an unreachable branch"
    );
}
