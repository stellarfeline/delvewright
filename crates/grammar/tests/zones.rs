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
use delvewright_grammar::ir::{Paint, Program};
use delvewright_grammar::library::bell::cliff_road::{MIN_DROP, MIN_GULF};
use delvewright_grammar::library::{cliff_road, gate_ward, hall_keep, watch_bay};
use delvewright_grammar::{Box3, ExpandOptions, Expansion, expand};

use support::{
    connected, ends, expand_at, indexed, passable, sees, solid, standable, standable_cells,
};

/// **Z1.** The crag: three cells of gulf, the road, and the rock it is cut into,
/// over eight courses of drop.
const CLIFF_REGION: Box3 = Box3::at_origin([12, 12, 36]);
/// The seed the four-niche road is pinned to.
const CLIFF_SEED: u64 = 4;

/// **Z2.** Gatehouse: twelve cells of threshold room, sixteen of gated passage.
const WARD_REGION: Box3 = Box3::at_origin([11, 7, 28]);
/// Neither piece of the gatehouse draws from the seed; it is stated, not chosen.
const WARD_SEED: u64 = 1;

/// **Z5.** Keep: twelve cells of stores, twelve of threshold, sixteen of hall.
const KEEP_REGION: Box3 = Box3::at_origin([11, 9, 40]);
/// The storeroom's tell is a seeded draw; this is the pinned fixture's seed.
const KEEP_SEED: u64 = 1;

/// The odd barrel's block (`store_room`'s `barrel_unbanded` default).
const TELL_BLOCK: &str = "minecraft:spruce_log";

fn zones() -> Vec<(Program, Box3, u64)> {
    vec![
        (cliff_road(), CLIFF_REGION, CLIFF_SEED),
        (gate_ward(), WARD_REGION, WARD_SEED),
        (hall_keep(), KEEP_REGION, KEEP_SEED),
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
    for (program, _, _) in zones() {
        program
            .validate()
            .unwrap_or_else(|e| panic!("{}: {e}", program.name));
    }
}

/// ADR-0006 at zone scale: same program, region and seed, byte-identical model
/// *and* anchors.
#[test]
fn every_zone_expands_byte_identically_twice() {
    for (program, region, seed) in zones() {
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
    for (program, _, _) in zones() {
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
    for (base, region, seed) in zones() {
        let mut restyled = base.clone();
        let roles: Vec<String> = base.palette.keys().cloned().collect();
        assert!(
            roles.len() >= 2,
            "{} binds {} roles",
            base.name,
            roles.len()
        );
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
/// Binding: the standable cells of each zone — 40 (Z1), 239 (Z2), 368 (Z5).
#[test]
fn every_zone_is_walkable_end_to_end() {
    for (program, region, seed) in zones() {
        let out = expand_at(&program, region, seed);
        let cells = standable_cells(&out.model);
        let (entry, exit) = ends(&out.model);
        assert!(
            !entry.is_empty() && !exit.is_empty(),
            "{}: nowhere to stand at the ends ({} standable cells)",
            program.name,
            cells.len()
        );
        assert!(
            connected(&cells, &entry, &exit),
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
    let cliff = expand_at(&cliff_road(), CLIFF_REGION, CLIFF_SEED);
    assert_eq!(indexed(&cliff.anchors, "niche").len(), 4);
    assert_eq!(indexed(&cliff.anchors, "niche-watch").len(), 4);
    assert_eq!(standable_cells(&cliff.model).len(), 40);

    let ward = expand_at(&gate_ward(), WARD_REGION, WARD_SEED);
    assert_eq!(span_cells(&ward).len(), 27);
    assert_eq!(approach_cells(&ward).len(), 184);
    assert_eq!(standable_cells(&ward.model).len(), 239);

    let keep = expand_at(&hall_keep(), KEEP_REGION, KEEP_SEED);
    assert_eq!(indexed(&keep.anchors, "perch").len(), 4);
    assert_eq!(approach_cells(&keep).len(), 205);
    assert_eq!(standable_cells(&keep.model).len(), 368);
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
/// Binding: 27 span cells, 239 standable cells re-walked without them.
#[test]
fn the_hazard_span_cannot_be_walked_round() {
    let out = expand_at(&gate_ward(), WARD_REGION, WARD_SEED);
    let cells = standable_cells(&out.model);
    let gate = out.anchors["anchor/gate"].pos;
    let (entry, exit) = ends(&out.model);
    assert!(connected(&cells, &entry, &exit));

    let span = span_cells(&out);
    assert_eq!(span.len(), 27, "the span has cells to close");
    let cut: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| (c[2] - gate[2]).abs() > 1)
        .collect();
    assert!(
        !connected(&cut, &entry, &exit),
        "with the portcullis span closed the gatehouse still connects end to end — \
         there is a way round the hazard, so nothing about its timing matters"
    );
}

/// Gate 2: the bay still sees the whole span once the zone is built around it.
/// `watch_bay` proves this in a bare box; the composition is a new box, and a
/// property re-proved on the assembled model is the only one a campaign can
/// rely on.
///
/// Binding: 27 span cells, each walked from the bay.
#[test]
fn the_bay_sees_the_whole_span_after_composition() {
    let out = expand_at(&gate_ward(), WARD_REGION, WARD_SEED);
    let watch = out.anchors["anchor/watch"].pos;
    assert!(standable(&out.model, watch), "the watch cell {watch:?}");
    let span = span_cells(&out);
    assert_eq!(span.len(), 27);
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
    assert!(connected(&cells, &entry, &exit));
}

/// Gate 3, and the one composition can most easily break: the alcove is blind
/// from **the whole zone**, not merely from the threshold piece's own approach.
/// The gatehouse gives the player 184 places to stand before the wall, against
/// the 54 the piece's own gate examined.
///
/// Binding: 184 approach cells.
#[test]
fn the_alcove_is_blind_from_the_whole_gatehouse() {
    let out = expand_at(&gate_ward(), WARD_REGION, WARD_SEED);
    let alcove = out.anchors["anchor/alcove"].pos;
    assert!(standable(&out.model, alcove), "the alcove cell {alcove:?}");
    let approach = approach_cells(&out);
    assert_eq!(approach.len(), 184, "the zone-scale approach set");
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
        seen, 147,
        "the doorway was widened straight over the alcove and the zone-scale \
         blindness check still reported it hidden — the gate proves nothing"
    );
}

// ---------------------------------------------------------------------------
// Z5 — the Great Hall and Keep
// ---------------------------------------------------------------------------

/// Gate 1: the doorway is the only way from the hall into the stores. Cut its
/// column and the keep is two rooms.
///
/// Binding: 205 approach cells, 162 cells behind the wall, 368 standable.
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
    assert_eq!(inside.len(), 162);
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
        2,
        "the truss was closed across the nave and the door still saw every perch — \
         the gate proves nothing"
    );

    // The control: the timbers are a ceiling, not a wall — the keep still walks
    // end to end, so what the gate caught is blindness.
    let cells = standable_cells(&out.model);
    let (entry, exit) = ends(&out.model);
    assert!(connected(&cells, &entry, &exit));
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
        seen, 152,
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
/// Binding: 8 seeds × the whole zone's cells searched for the tell block; 5
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

// ---------------------------------------------------------------------------
// What composition itself has to get right
// ---------------------------------------------------------------------------

/// The pieces stand in travel order along the zone's own axis, and every anchor
/// they declare faces down it. This is the cheap, total check that no piece was
/// turned: a rotated piece declares its anchors facing `west`, and its wall
/// across the route.
///
/// Binding: 8 anchors (Z1: 4 niches, 4 watch cells), 4 (Z2), 9 (Z5).
#[test]
fn the_pieces_stand_in_travel_order() {
    let ward = expand_at(&gate_ward(), WARD_REGION, WARD_SEED);
    let z = |out: &Expansion, name: &str| out.anchors[name].pos[2];
    assert!(
        z(&ward, "anchor/alcove") < z(&ward, "anchor/gate")
            && z(&ward, "anchor/gate") < z(&ward, "anchor/watch"),
        "the gatehouse's pieces are out of travel order: {:#?}",
        ward.anchors
    );

    let keep = expand_at(&hall_keep(), KEEP_REGION, KEEP_SEED);
    assert!(
        z(&keep, "anchor/store-line") < z(&keep, "anchor/threshold")
            && z(&keep, "anchor/threshold") < z(&keep, "anchor/hall-door"),
        "the keep's pieces are out of travel order: {:#?}",
        keep.anchors
    );

    // Every anchor faces the way its rule meant it to. Down travel (`north`)
    // for all of them except the three the vocabulary deliberately aims *across*
    // the route through a reorient — the recess, the alcove and the tell — which
    // face `west` precisely because the route runs north.
    let road = expand_at(&cliff_road(), CLIFF_REGION, CLIFF_SEED);
    let mut checked = 0;
    for out in [&road, &ward, &keep] {
        for (name, anchor) in &out.anchors {
            let across = name == "anchor/alcove"
                || name == "anchor/tell"
                || (name.starts_with("anchor/niche-") && !name.starts_with("anchor/niche-watch-"));
            let want = if across { "west" } else { "north" };
            assert_eq!(
                anchor.facing.as_str(),
                want,
                "{name} faces the wrong way — the piece that declared it was turned"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 21, "the gate checked {checked} anchors");
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
}

/// The limit of the seam, stated as a test rather than as a comment: an include
/// does **not** rename anchors, because an anchor name is the campaign's
/// contract. So a zone that includes the same piece twice gets two declarations
/// of one name, and expansion refuses loudly instead of letting the second
/// quietly win.
///
/// A zone that genuinely needs two watch bays therefore needs an anchor-
/// namespace primitive on `mark`; this is what its absence looks like.
#[test]
fn including_one_piece_twice_is_a_loud_anchor_collision() {
    use delvewright_grammar::compose::{entry, include};
    use delvewright_grammar::geom::Axis;
    use delvewright_grammar::ir::{Alternative, Node, Size, Split};

    let bay = watch_bay();
    let twice = Program::new("two_bays", "plan").rule_alts(
        "plan",
        vec![Alternative::new(Node::Split(Split {
            axis: Axis::Z,
            sizes: vec![Size::rel(1), Size::rel(1)],
            rounding: Default::default(),
            repeat: false,
            orient: Default::default(),
            children: vec![
                Node::call(&entry("first", &bay)),
                Node::call(&entry("second", &bay)),
            ],
        }))],
    );
    let twice = include(twice, &bay, "first").unwrap();
    let twice = include(twice, &bay, "second").unwrap();
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
}
