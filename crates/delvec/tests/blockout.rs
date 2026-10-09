//! **The derived blockout, and its independent observer** (spec-0049 §5).
//!
//! Two halves, and the second is the one that makes the first mean anything.
//!
//! The first half is that the whole map derives, builds and walks from four
//! JSON documents with no authored geometry anywhere — `tests/fixtures/blockout`
//! carries a five-place graph with a walk, a stair, a designed fall that closes
//! a loop, a barred way the keeper opens and a vista, and nothing that describes
//! a block.
//!
//! The second half is spec-0049's acceptance criterion 8, and it is the reason
//! this file is shaped the way it is: **every red here is produced by a
//! deliberately perturbed DERIVATION, never by hand-authored bytes.** A check
//! that replays the derivation's own arithmetic agrees with it by construction,
//! however wrong both are; the only demonstration that `DW0836`, `DW0837` and
//! `DW0838` are observing the mass rather than reciting it is to make the
//! derivation build the map wrong in a named way and watch them say so. So each
//! perturbation test asserts three things: that the code fires under the defect,
//! that it does NOT fire without it, and what the check bound to while deciding.

mod common;

use std::collections::BTreeMap;

use common::source_scan;

use delvec::compiler::blockout::{self, Perturb};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::siteplan::PlacedBox;
use delvewright_dsl::{Campaign, Severity};

/// The site-plan fixture: five places, zero authored geometry.
fn fixture_dir() -> std::path::PathBuf {
    common::repo_root().join("crates/delvec/tests/fixtures/blockout")
}

fn campaign() -> Campaign {
    let loaded = delvec::compiler::load::load_campaign_dir(&fixture_dir())
        .expect("the blockout fixture is readable");
    delvewright_dsl::parse_campaign(&loaded.raw).expect("the blockout fixture parses")
}

fn prefabs() -> PrefabRegistry {
    PrefabRegistry::load_dir(&common::prefabs_dir()).expect("the prefab library loads")
}

/// Derive the blockout under `perturb`, assemble it, and run the battery.
///
/// The same two steps `emit::build_with_warnings` takes, in the same order, over
/// the same models — so what this file proves is what a build proves.
fn battery_under(perturb: Perturb) -> (blockout::Battery, Vec<String>) {
    let c = campaign();
    let reg = prefabs();
    let plan = Plan::build_with(&c, &reg, perturb).expect("the blockout fixture plans");
    let structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let blocks = delvec::compiler::assembled::assembled_blocks(&plan, &structures);
    let battery = blockout::check(&plan, &blocks).expect("a site-plan campaign has a blockout");
    let codes = battery
        .findings
        .iter()
        .map(|(_, d)| d.code.clone())
        .collect();
    (battery, codes)
}

fn errors(b: &blockout::Battery) -> Vec<String> {
    b.findings
        .iter()
        .filter(|(_, d)| d.severity == Severity::Error)
        .map(|(_, d)| d.code.clone())
        .collect()
}

fn message_for(b: &blockout::Battery, code: &str) -> String {
    b.findings
        .iter()
        .find(|(_, d)| d.code == code)
        .map(|(_, d)| d.message.clone())
        .unwrap_or_else(|| panic!("no `{code}` among {:?}", errors(b)))
}

// ---------------------------------------------------------------------------
// The whole map, derived
// ---------------------------------------------------------------------------

/// The blockout derives, assembles and passes its own battery — and the battery
/// says what it examined.
///
/// The binding assertions are the point of the test as much as the green is: a
/// battery that examined nothing would pass too, and a green that rests on a
/// zero binding is the first vacuity mode `CLAUDE.md` names.
#[test]
fn the_derived_whole_is_green_and_states_what_it_bound_to() {
    let (b, _) = battery_under(Perturb::none());
    assert!(
        errors(&b).is_empty(),
        "the unperturbed derivation must satisfy its own observer: {:?}\n{}",
        errors(&b),
        b.findings
            .iter()
            .map(|(_, d)| d.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
    let k = b.binding;
    assert_eq!(k.seams, 7, "seven traversal connections are allocated");
    assert_eq!(
        k.walls, 6,
        "seven seams over six walls — the stair and the fall pierce one wall, and \
         the undercroft's own stair pierces the cell's floor"
    );
    assert_eq!(k.nodes, 7, "seven places are proven reached");
    assert!(
        k.standable > 500,
        "the crossing check classified {} standable cell(s), which is not a map",
        k.standable
    );
    assert_eq!(k.pairs, 21, "seven places make twenty-one unordered pairs");
    assert_eq!(k.sightlines, 1);
    assert_eq!(k.identities, 6);
    assert_eq!(
        k.identities_declared_only, 2,
        "the two region-extent identities have no byte-side referent"
    );
    assert_eq!(k.legs, 3, "the critical path has three legs");
}

/// The derivation itself binds, and its output is mass rather than a promise.
#[test]
fn the_derivation_states_what_it_massed() {
    let c = campaign();
    let reg = prefabs();
    let plan = Plan::build(&c, &reg).expect("the blockout fixture plans");
    let b = plan
        .blockout
        .as_ref()
        .expect("a site plan derives a blockout");
    let k = b.binding;
    assert_eq!(k.boxes, 7);
    assert_eq!(k.seams, 7);
    assert_eq!(
        k.stairs, 2,
        "two connections are built out of treads — one across a wall, one down \
         through a punched floor"
    );
    assert_eq!(k.barred, 1, "one way is sealed at world load");
    assert_eq!(k.volumes, 2);
    assert!(
        k.cells > 10_000,
        "a whole map is more than {} cells",
        k.cells
    );
    // Every write is inside what the game will accept, because a `fill` the
    // server refuses fails in a function nobody reads.
    let area = plan
        .areas
        .iter()
        .find(|a| a.area_id == delvewright_dsl::SITE_AREA)
        .expect("the site plan places one area");
    assert!(!area.mass.is_empty());
    for m in &area.mass {
        let n: u64 = (0..3)
            .map(|i| (i64::from(m.to[i]) - i64::from(m.from[i]) + 1).unsigned_abs())
            .product();
        assert!(
            n <= blockout::MAX_FILL_CELLS,
            "a {n}-cell fill is more than vanilla will write in one command"
        );
    }
    assert!(
        area.pieces.iter().all(|p| p.templates.is_empty()),
        "a derived piece carries no structure template — its blocks are the mass"
    );
}

// ---------------------------------------------------------------------------
// The perturbations (spec-0049 §13.8)
// ---------------------------------------------------------------------------

/// `DW0836`: the derivation cuts every hole one cell over, and the observer
/// catches it from both directions.
#[test]
fn a_slid_opening_reddens_dw0836() {
    let (clean, _) = battery_under(Perturb::none());
    assert!(
        !errors(&clean).contains(&"DW0836".to_string()),
        "the unperturbed derivation builds the openings the plan allocated"
    );
    let (b, _) = battery_under(Perturb {
        slide_openings: 1,
        ..Perturb::none()
    });
    assert!(
        errors(&b).contains(&"DW0836".to_string()),
        "a hole cut one cell over is a hole the plan did not allocate: {:?}",
        errors(&b)
    );
    let m = message_for(&b, "DW0836");
    assert!(
        m.contains("still solid") || m.contains("allocated no seam for"),
        "the refusal must say which way it disagrees: {m}"
    );
    assert_eq!(
        b.binding.seams, 7,
        "the binding is stated even when the check refuses"
    );
}

/// `DW0836`'s other half: the mass is laid at a height the plan did not choose,
/// and nothing at stage 4 could ever have seen it.
#[test]
fn a_sunk_place_reddens_dw0836_on_the_realized_rise() {
    let (b, _) = battery_under(Perturb {
        sink: Some("node/loft".to_string()),
        ..Perturb::none()
    });
    let m = message_for(&b, "DW0836");
    assert!(
        m.contains("spans a climb of"),
        "the realized rise must be the thing that disagreed: {m}"
    );
    // The same defect moves a datum, so the identity's SECOND call site — the
    // one that exists precisely because a plan-time green cannot see this —
    // refuses as well.
    assert!(
        errors(&b).contains(&"DW0833".to_string()),
        "the brief's loft datum is measured off the bytes: {:?}",
        errors(&b)
    );
    assert!(
        message_for(&b, "DW0833").contains("BUILT world"),
        "the second call site must say which world it measured"
    );
}

/// `DW0837`: a place whose interior the derivation never cleared.
#[test]
fn a_bricked_up_place_reddens_dw0837() {
    let (clean, _) = battery_under(Perturb::none());
    assert!(!errors(&clean).contains(&"DW0837".to_string()));
    let (b, _) = battery_under(Perturb {
        brick_up: Some("node/exit".to_string()),
        ..Perturb::none()
    });
    assert!(
        errors(&b).contains(&"DW0837".to_string()),
        "a place with nowhere to stand is a place nobody reaches: {:?}",
        errors(&b)
    );
    let m = message_for(&b, "DW0837");
    assert!(
        m.contains("node/exit") && m.contains("standable cell(s)"),
        "the refusal names the place and what it offered: {m}"
    );
    assert_eq!(b.binding.nodes, 7, "all seven places were examined");
}

/// `DW0838`: walls one course tall, so a body hops between two places somewhere
/// the plan allocated nothing.
///
/// This is also the test that demonstrates why the check is made over PATHS
/// rather than over steps: the crossing is three moves long — floor, wall top,
/// the next floor — and no single one of them joins two owned cells, because the
/// plan's own `DW0828` puts exactly one cell between any two boxes.
#[test]
fn a_low_wall_reddens_dw0838() {
    let (clean, _) = battery_under(Perturb::none());
    assert!(
        !errors(&clean).contains(&"DW0838".to_string()),
        "with the walls built, the only ways between places are the allocated ones"
    );
    let (b, _) = battery_under(Perturb {
        short_walls: true,
        ..Perturb::none()
    });
    assert!(
        errors(&b).contains(&"DW0838".to_string()),
        "a wall a body can climb is a seam that was discovered: {:?}",
        errors(&b)
    );
    // On an `open` site a wall a body can climb also lets it out onto the
    // terrain, so both shapes of the claim fire; the pair shape is the one this
    // test is about.
    let pair = b
        .findings
        .iter()
        .filter(|(_, d)| d.code == "DW0838")
        .map(|(_, d)| d.message.clone())
        .find(|m| m.contains("allocated no seam for"))
        .unwrap_or_else(|| panic!("no pair-shaped DW0838 among {:?}", errors(&b)));
    assert!(
        pair.contains("can still walk to"),
        "the refusal names both places and a witness cell: {pair}"
    );
    assert_eq!(b.binding.pairs, 21);
}

// ---------------------------------------------------------------------------
// The headroom measure, and the run a stair really has
// ---------------------------------------------------------------------------

/// The derived world of the unperturbed fixture, for tests that read blocks
/// rather than findings.
fn derived_world() -> (Vec<PlacedBox>, delvec::compiler::nav::World) {
    let c = campaign();
    let reg = prefabs();
    let plan = Plan::build(&c, &reg).expect("the blockout fixture plans");
    let structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let world = delvec::compiler::nav::World::from_plan(&plan, &structures);
    let boxes = plan
        .blockout
        .as_ref()
        .expect("a site plan derives a blockout")
        .boxes
        .clone();
    (boxes, world)
}

fn box_of<'a>(boxes: &'a [PlacedBox], node: &str) -> &'a PlacedBox {
    boxes
        .iter()
        .find(|b| b.node.0 == node)
        .unwrap_or_else(|| panic!("`{node}` is a place this fixture has"))
}

fn cell(c: [i64; 3]) -> [i32; 3] {
    [c[0] as i32, c[1] as i32, c[2] as i32]
}

/// **`DW0833`'s headroom is a PLACE, not one column of it.**
///
/// The condition is asserted rather than assumed, and that is the whole
/// discipline of this test: the hall's own centre column carries the derived
/// stair's treads, because a stair arrives at its seam and walks back through
/// the middle of the room. Counting upward from `centre()` therefore answered
/// **0** for a room whose ceiling is exactly where the plan put it. If the
/// fixture's stair ever moves off the middle this assertion fails rather than
/// the test quietly becoming vacuous.
#[test]
fn the_headroom_measure_reads_the_place_and_not_its_centre_column() {
    let (boxes, world) = derived_world();
    let hall = box_of(&boxes, "node/hall");
    assert!(
        !world.is_clear(cell(hall.centre())),
        "the hall's centre column carries the stair's treads — without that this \
         test proves nothing about which column was read"
    );
    let (b, _) = battery_under(Perturb::none());
    assert!(
        !errors(&b).contains(&"DW0833".to_string()),
        "a place whose ceiling is where the plan put it keeps its height \
         identity, whatever the plan hosts on its floor: {:?}",
        errors(&b)
    );
    assert_eq!(
        b.binding.identities, 6,
        "and the height identities were examined rather than skipped"
    );
}

/// `DW0833`: a ceiling closed one course into the play space, and nothing else
/// moved.
///
/// The perturbation is chosen so that only the check under test can see it. The
/// floor stays where the plan put it, so `DW0836`'s realized rise is unmoved;
/// the place stays walkable, so `DW0837` is unmoved; the openings are untouched,
/// so `DW0836`'s seam half is unmoved. A headroom check that reddened every box
/// would pass a test like this by accident, which is why the assertion below is
/// over the WHOLE error list and not over one code.
#[test]
fn a_low_ceiling_reddens_dw0833_on_the_headroom() {
    let (clean, _) = battery_under(Perturb::none());
    assert!(
        !errors(&clean).contains(&"DW0833".to_string()),
        "the unperturbed derivation keeps the brief's numbers"
    );
    let (b, _) = battery_under(Perturb {
        low_ceiling: Some("node/landing".to_string()),
        ..Perturb::none()
    });
    assert_eq!(
        errors(&b),
        vec!["DW0833".to_string()],
        "a course of ceiling is a height defect and nothing else"
    );
    let m = message_for(&b, "DW0833");
    assert!(
        m.contains("fact/landing-height") && m.contains("measured 3"),
        "the refusal names the fact and the figure it measured: {m}"
    );
    assert!(
        m.contains("the PLAN may have given this place something to hold"),
        "and it names BOTH ways the mass can disagree, not only the derivation: {m}"
    );
    assert_eq!(
        b.binding.identities, 6,
        "the binding is stated even when the check refuses"
    );
}

/// **A stair down through a punched floor is a whole run, and a body walks it
/// to the floor.**
///
/// The run of such a stair starts at the hole and leaves along one side of it,
/// so what it has is the room on that side plus the hole's own width — never the
/// host's whole extent. Chosen against the extent, the gentle 1:2 standard
/// "fit" a run the undercroft does not have and the courses that fell off the
/// far wall were dropped in silence: the ladder lost its bottom two treads, the
/// body could climb IN from above and stand on the stair, and the battery stayed
/// green over a room whose floor nobody could reach — because a place counts as
/// reached the moment a body stands anywhere inside it.
///
/// Both halves are asserted, and the first is the one that was silently false:
/// every course of the climb exists, and the walk plane itself is reachable from
/// the place above.
#[test]
fn a_stair_down_through_a_punched_floor_is_a_whole_run() {
    let (boxes, world) = derived_world();
    let under = box_of(&boxes, "node/undercroft");
    let cellar_above = box_of(&boxes, "node/cell");
    let (lo, hi) = under.space();

    // Every course of the climb is there: for each block of rise between the
    // walk plane and the floor the hole is cut in, some cell of the place is
    // standable at that height. A dropped course is a gap in this ladder.
    for h in 1..=i64::from(under.clearance) {
        let y = under.floor + h;
        let found =
            (lo[2]..=hi[2]).any(|z| (lo[0]..=hi[0]).any(|x| world.is_standable(cell([x, y, z]))));
        assert!(
            found,
            "no tread stands {h} block(s) over `node/undercroft`'s walk plane — \
             the run was laid short"
        );
    }

    // And the walk plane is reachable from the floor of the place above, which
    // is what the climb is for.
    let (alo, ahi) = cellar_above.space();
    let seeds: Vec<[i32; 3]> = (alo[2]..=ahi[2])
        .flat_map(|z| (alo[0]..=ahi[0]).map(move |x| [x, alo[1], z]))
        .map(cell)
        .filter(|c| world.is_standable(*c))
        .collect();
    assert!(
        !seeds.is_empty(),
        "the place above offers somewhere to start"
    );
    let reached = world.reachable_walkable(&seeds);
    let landed = (lo[2]..=hi[2])
        .flat_map(|z| (lo[0]..=hi[0]).map(move |x| [x, under.floor, z]))
        .any(|c| reached.contains(&cell(c)));
    assert!(
        landed,
        "no body standing on `node/cell`'s floor can reach `node/undercroft`'s \
         own walk plane over the step rule"
    );
}

/// **A stair down through a punched floor can be climbed back OUT.**
///
/// The run starts under the hole and walks back under the floor it pierces, so
/// its upper courses stand where the floor above would take a climbing body's
/// head: a course whose feet are within a body's height of that floor is a step
/// no body can stand on, and the stair is then a way down and never a way up —
/// the place under it is a place a body falls into and cannot leave (`DW0921`).
/// The hole the derivation cuts therefore clears the headroom over every course
/// of the run, not just the cells the plan allocated.
///
/// Judged by the movement relation `DW0921` floods (`nav::World::body_moves`), from
/// every standable cell of the lower place's walk plane — never a second model
/// of how a body climbs.
///
/// The fixture's own cellar climbs four whole courses under a three-cell hole,
/// and every course a head can reach is already under the hole, so it cannot
/// tell the two derivations apart. The cellar is therefore lowered into the
/// shape that can: a floor at `y` 60 under three cells of headroom, a climb of
/// three that the gentle 1:2 standard fits in six half-block courses — the
/// fourth of which stands one and a half blocks up, under the floor above. Its
/// tunnel moves with it, so the walk between them stays a walk.
#[test]
fn a_stair_down_through_a_punched_floor_can_be_climbed_back_out() {
    let mut loaded = delvec::compiler::load::load_campaign_dir(&fixture_dir())
        .expect("the blockout fixture is readable");
    let mut plan_doc: serde_json::Value = serde_json::from_str(
        loaded
            .raw
            .site_plan
            .as_deref()
            .expect("the blockout fixture carries a site plan"),
    )
    .expect("the site plan is JSON");
    let mut moved = 0;
    for b in plan_doc["content"]["boxes"]
        .as_array_mut()
        .expect("a site plan has boxes")
    {
        if b["node"] == "node/undercroft" || b["node"] == "node/tunnel" {
            b["floor"] = serde_json::json!({ "y": 60 });
            b["ceiling"] = serde_json::json!({ "clearance": 3 });
            moved += 1;
        }
    }
    assert_eq!(moved, 2, "the cellar and its tunnel are both re-seated");
    loaded.raw.site_plan = Some(plan_doc.to_string());
    let c = delvewright_dsl::parse_campaign(&loaded.raw).expect("the re-seated fixture parses");
    let plan = Plan::build(&c, &prefabs()).expect("the re-seated fixture plans");
    let structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let world = delvec::compiler::nav::World::from_plan(&plan, &structures);
    let blockout = plan
        .blockout
        .as_ref()
        .expect("a site plan derives a blockout");
    assert!(
        blockout.binding.stairwell_cells > 0,
        "the re-seated cellar is the shape whose run reaches past its hole: {}",
        blockout.binding.line()
    );
    let battery = blockout::check(
        &plan,
        &delvec::compiler::assembled::assembled_blocks(&plan, &structures),
    )
    .expect("a site-plan campaign has a blockout");
    assert_eq!(
        errors(&battery),
        Vec::<String>::new(),
        "the stairwell is a hole the observers accept"
    );
    let boxes = blockout.boxes.clone();
    let under = box_of(&boxes, "node/undercroft");
    let above = box_of(&boxes, "node/cell");
    let (lo, hi) = under.space();
    let (alo, ahi) = above.space();
    let floor: Vec<[i32; 3]> = (lo[2]..=hi[2])
        .flat_map(|z| (lo[0]..=hi[0]).map(move |x| [x, under.floor, z]))
        .map(cell)
        .filter(|c| world.is_standable(*c))
        .collect();
    assert!(
        !floor.is_empty(),
        "`node/undercroft` offers a walk plane to start from"
    );
    let in_above = |c: [i32; 3]| {
        (0..3).all(|i| i64::from(c[i]) >= alo[i] && i64::from(c[i]) <= ahi[i])
            && i64::from(c[1]) == above.floor
    };
    let mut seen: std::collections::BTreeSet<[i32; 3]> = floor.iter().copied().collect();
    let mut queue: std::collections::VecDeque<[i32; 3]> = seen.iter().copied().collect();
    let mut out = false;
    while let Some(cur) = queue.pop_front() {
        if in_above(cur) {
            out = true;
            break;
        }
        for n in world.body_moves(cur) {
            if seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    assert!(
        out,
        "no body on `node/undercroft`'s walk plane ({} standable cell(s)) can walk, \
         fall, jump or swim onto `node/cell`'s floor: the stair it came down is not a \
         way back up ({} cell(s) reached)",
        floor.len(),
        seen.len()
    );
}

/// `DW0836`: a floor cut over a stair's whole run, and not only where a
/// climbing body needs it, is a hole wider than the plan allocated.
///
/// Claim 2 admits a stair's stairwell — the open cells off its hole that a body
/// on the stair stands in, puts its head in, or sweeps in a jump — and this is
/// what shows that admission refuses anything. The fixture's cellar climbs
/// inside its hole, so the unperturbed derivation cuts no stairwell at all and
/// every cell the perturbation opens is a leak.
#[test]
fn an_open_stairwell_reddens_dw0836() {
    let (clean, _) = battery_under(Perturb::none());
    assert!(
        !errors(&clean).contains(&"DW0836".to_string()),
        "the measured stairwell is a hole the observer accepts"
    );
    let (b, _) = battery_under(Perturb {
        open_stairwells: true,
        ..Perturb::none()
    });
    assert_eq!(
        errors(&b),
        vec!["DW0836".to_string()],
        "a floor opened over courses no head reaches is a wider hole and nothing else"
    );
    let m = message_for(&b, "DW0836");
    assert!(
        m.contains("edge/cell-undercroft"),
        "the refusal names the stair's own wall: {m}"
    );
}

/// The stair across a VERTICAL face is untouched, and the assertion is over the
/// blocks rather than over a hash so a reader can see which stair and where.
///
/// Such a run starts against the wall the seam is in and walks the whole
/// footprint, so the span its pitch is chosen against IS the host's extent —
/// which is what it always was. The hall's climb of five is still the gentle
/// 1:2 standard, ten courses back from the east wall at x 24, and the eleventh
/// cell is still room.
#[test]
fn the_stair_across_a_wall_still_spends_the_whole_footprint() {
    let (boxes, world) = derived_world();
    let hall = box_of(&boxes, "node/hall");
    assert!(
        !world.is_clear(cell([15, hall.floor, 10])),
        "the tenth course of the hall's ramp stands at x 15"
    );
    assert!(
        world.is_standable(cell([14, hall.floor, 10])),
        "and the cell beyond it is floor: a ten-course run, not eleven"
    );
}

// ---------------------------------------------------------------------------
// The advisories
// ---------------------------------------------------------------------------

/// `DW0821`: the vista does not read in the blockout, and the walk sheet is told
/// every cell that stops it — not the first.
#[test]
fn dw0821_warns_and_names_every_blocking_cell() {
    let (b, codes) = battery_under(Perturb::none());
    assert!(codes.contains(&"DW0821".to_string()));
    let (_, d) = b
        .findings
        .iter()
        .find(|(_, d)| d.code == "DW0821")
        .expect("the fixture's sightline crosses a shell");
    assert_eq!(
        d.severity,
        Severity::Warning,
        "an error here would force hand-shaped massing ahead of the walk evidence \
         spec-0049 §5.1 reserves it for"
    );
    // Three cells of one wall, all of them named.
    assert!(
        d.message
            .contains("[23, 68, 11], [24, 68, 11], [25, 68, 11]"),
        "every blocking cell is named: {}",
        d.message
    );
}

/// `DW0822`'s second call site: the route the built map really is, beside the
/// projection the graph made of it.
#[test]
fn dw0822_measures_the_built_route_at_the_second_call_site() {
    let (b, _) = battery_under(Perturb::none());
    let (_, d) = b
        .findings
        .iter()
        .find(|(_, d)| d.code == "DW0822")
        .expect("a critical path with legs is measured");
    assert_eq!(
        d.severity,
        Severity::Warning,
        "the figure carries no threshold"
    );
    assert!(
        d.message.contains("MEASURES") && d.message.contains("3 leg(s)"),
        "the measurement states its own binding: {}",
        d.message
    );
    assert!(
        !d.message.contains("could not route"),
        "every leg of the fixture's critical path routes over the built blockout: {}",
        d.message
    );
}

// ---------------------------------------------------------------------------
// Determinism (spec-0049 §13.4)
// ---------------------------------------------------------------------------

/// The same plan derives the same mass twice, and **the seed reaches none of
/// it**.
///
/// The second half is the one worth stating: `world.seed` is what makes a pool
/// area's layout what it is, and a blockout has no draw to make — so a campaign
/// whose map is a site plan reproduces without the seed mattering at all.
#[test]
fn the_derivation_is_deterministic_and_seedless() {
    let mass_of = |seed: u64| -> Vec<String> {
        let mut c = campaign();
        c.world.content.seed = seed;
        let reg = prefabs();
        let plan = Plan::build(&c, &reg).expect("the blockout fixture plans");
        plan.areas
            .iter()
            .find(|a| a.area_id == delvewright_dsl::SITE_AREA)
            .expect("one site area")
            .mass
            .iter()
            .map(|m| format!("{:?}..{:?} {}", m.from, m.to, m.block))
            .collect()
    };
    let a = mass_of(20260821);
    let b = mass_of(20260821);
    assert_eq!(a, b, "two derivations of one plan are the same mass");
    let c = mass_of(1);
    assert_eq!(
        a, c,
        "changing the seed changes no blockout byte — the derivation never draws"
    );
    assert!(!a.is_empty());
}

/// The production path is never perturbed.
///
/// The one thing [`Perturb`] could cost, asserted directly: `Plan::build` — the
/// constructor every build that is not a `--perturb` demonstration goes through
/// — asks for no defect, and a build made through it is byte-identical to one
/// made by naming [`Perturb::none`] explicitly.
#[test]
fn blockout_derivation_is_never_perturbed_in_production() {
    assert!(Perturb::none().is_none());
    assert!(Perturb::default().is_none());
    let c = campaign();
    let reg = prefabs();
    let via_build = Plan::build(&c, &reg).expect("plans");
    let via_none = Plan::build_with(&c, &reg, Perturb::none()).expect("plans");
    let mass = |p: &Plan| -> Vec<String> {
        p.areas
            .iter()
            .flat_map(|a| a.mass.iter())
            .map(|m| format!("{:?}..{:?} {}", m.from, m.to, m.block))
            .collect()
    };
    assert_eq!(mass(&via_build), mass(&via_none));
}

/// **The flag is the only way in**, asserted over the sources rather than
/// remembered.
///
/// `Plan::build_with` is the parameterised constructor; every other build in
/// this engine reaches the derivation through `Plan::build`, which passes
/// `Perturb::none()` as a literal. The test above proves the two agree; this one
/// proves nothing else calls the parameterised one. Without it, a second caller
/// could acquire a perturbation quietly and every assertion about the shipped
/// derivation would still be green — a claim about a smaller world than the
/// engine has.
///
/// It is a source scan because the property is about CALLERS, and a caller that
/// exists is invisible to any amount of behavioural testing of the callee. The
/// population is stated so a zero cannot pass for a pass.
#[test]
fn the_parameterised_derivation_has_exactly_one_production_caller() {
    let root = common::repo_root();
    let mut files = Vec::new();
    for crate_dir in std::fs::read_dir(root.join("crates")).expect("crates/ is readable") {
        let src = crate_dir.expect("a crate entry").path().join("src");
        if src.is_dir() {
            collect_rs(&src, &mut files);
        }
    }
    // Directory order is the filesystem's; the assertion below is over a list.
    files.sort();
    assert!(
        files.len() > 20,
        "the scan found {} source file(s), which is not this workspace",
        files.len()
    );

    // Every line that CALLS the parameterised constructor: `build_with_warnings`
    // is a different function and is excluded by name, and a `fn` line is the
    // definition rather than a call.
    let mut callers: Vec<(String, String)> = Vec::new();
    let mut named_perturb: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for path in &files {
        let text = std::fs::read_to_string(path).expect("a source file is readable");
        let name = path
            .strip_prefix(&root)
            .expect("every scanned file is under the repo root")
            .to_string_lossy()
            .into_owned();
        for line in text.lines() {
            let t = line.trim();
            // Doc and ordinary comments describe the facility all over this
            // crate; the property is about code.
            if t.starts_with("//") {
                continue;
            }
            if t.contains("Perturb") {
                named_perturb.insert(name.clone());
            }
            if t.contains("build_with(") && source_scan::fn_name(t).is_none() {
                callers.push((name.clone(), t.to_string()));
            }
        }
    }

    // `plan/mod.rs` calls it from `Plan::build` with the literal that asks for
    // nothing; the binary's `cli/campaign.rs` calls it from the `--perturb`
    // arm. Nothing else may.
    let sites: Vec<&str> = callers.iter().map(|(f, _)| f.as_str()).collect();
    assert_eq!(
        sites,
        vec![
            "crates/delvec/src/cli/campaign.rs",
            "crates/delvec/src/compiler/plan/mod.rs"
        ],
        "the parameterised derivation acquired a caller: {callers:#?}"
    );
    assert!(
        callers[1].1.contains("Perturb::none()"),
        "`Plan::build` must pass the literal that asks for nothing: {}",
        callers[1].1
    );

    // And the facility is not NAMED anywhere else either — a file that mentions
    // `Perturb` in code is a file that could grow the second caller next.
    assert_eq!(
        named_perturb.iter().map(String::as_str).collect::<Vec<_>>(),
        vec![
            "crates/delvec/src/cli/campaign.rs",
            "crates/delvec/src/compiler/blockout.rs",
            "crates/delvec/src/compiler/plan/mod.rs",
        ],
        "binding: {} source file(s) scanned, {} call site(s) found",
        files.len(),
        callers.len()
    );
}

/// Recursively collect `*.rs` under `dir`.
fn collect_rs(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).expect("a source directory is readable") {
        let p = e.expect("a directory entry").path();
        if p.is_dir() {
            collect_rs(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// **Every field of [`Perturb`] is reachable from the command line**, checked by
/// the compiler rather than by a reader.
///
/// The destructuring below names all six fields with no `..` rest pattern, so a
/// seventh defect added to `Perturb` and not given a [`Knob`] does not compile
/// this test — which is the strongest form available for "the enumeration is
/// complete", and the one `CLAUDE.md` asks for over a hand-written list. The
/// assertions then say each field is actually MOVED by its knob, so an arm that
/// exists and does nothing is a red rather than a pass.
#[test]
fn every_perturb_field_has_a_knob() {
    use delvec::compiler::blockout::Knob;

    let place = "node/exit";
    let mut seen = 0;
    let (mut slid, mut sunk, mut short, mut bricked, mut low, mut walled, mut wells) =
        (false, false, false, false, false, false, false);
    let (mut buried, mut hollow) = (false, false);
    for knob in Knob::ALL {
        let p = knob
            .perturb(knob.takes_place().then_some(place))
            .expect("a knob's own `takes_place` answer satisfies its own constructor");
        assert!(
            !p.is_none(),
            "`{}` produced the unperturbed derivation, so it demonstrates nothing",
            knob.name()
        );
        // Exhaustive: no `..`. Adding a field to `Perturb` breaks the build here.
        let Perturb {
            slide_openings,
            sink,
            short_walls,
            brick_up,
            low_ceiling,
            wall_contacts,
            open_stairwells,
            bury_barred,
            hollow_edge,
        } = p;
        slid |= slide_openings != 0;
        sunk |= sink.is_some();
        short |= short_walls;
        bricked |= brick_up.is_some();
        low |= low_ceiling.is_some();
        walled |= wall_contacts;
        wells |= open_stairwells;
        buried |= bury_barred;
        hollow |= hollow_edge;
        seen += 1;
        assert!(
            knob.perturb(knob.takes_place().then_some("")).is_some(),
            "a knob's constructor must accept the arity its own `takes_place` states"
        );
        assert!(
            knob.perturb((!knob.takes_place()).then_some(place))
                .is_none(),
            "`{}` must refuse the arity its own `takes_place` denies, rather than \
             silently deriving a clean map",
            knob.name()
        );
    }
    assert_eq!(seen, Knob::ALL.len());
    assert!(
        slid && sunk && short && bricked && low && walled && wells && buried && hollow,
        "one of the nine fields is never set by any knob: slid={slid} sunk={sunk} \
         short={short} bricked={bricked} low={low} walled={walled} wells={wells} \
         buried={buried} hollow={hollow}"
    );
    // The spellings a creator types are unique and kebab-case, since the value
    // set is resolved by name.
    let names: std::collections::BTreeSet<&str> = Knob::ALL.iter().map(|k| k.name()).collect();
    assert_eq!(names.len(), Knob::ALL.len(), "two knobs share a spelling");
    for k in Knob::ALL {
        assert!(
            k.name().chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "`{}` is not a kebab-case value",
            k.name()
        );
        // A battery code is declared by the battery's module, or is the site
        // plan's identity code the battery re-measures at its second call site.
        let declared = delvewright_dsl::diagnostic::declared();
        let owner = declared
            .iter()
            .find(|d| d.code == k.documented_code())
            .map(|d| d.module);
        assert!(
            matches!(
                owner,
                Some("delvec::compiler::blockout" | "delvewright_dsl::siteplan")
            ),
            "`{}` names `{}`, declared by {owner:?}, which is not a blockout-battery code",
            k.name(),
            k.documented_code()
        );
    }
}

// ---------------------------------------------------------------------------
// The synthesized vocabulary (spec-0049 §5.2)
// ---------------------------------------------------------------------------

/// The quest, gate and shortcut machinery lands on massing nobody authored, and
/// does not know the difference.
///
/// Each assertion below is a piece of the existing engine reaching into a world
/// with no prefab in it: the campaign's spawn, an NPC's stand, a `reach-anchor`
/// objective's target, and an `open-gate`'s region — all resolved through
/// `plan.anchors`, which is the one map every consumer already goes through.
#[test]
fn the_synthesized_vocabulary_carries_the_unchanged_quest_layer() {
    let c = campaign();
    let reg = prefabs();
    let plan = Plan::build(&c, &reg).expect("plans");
    let area = delvewright_dsl::SITE_AREA.to_string();

    // The entry, resolved through the compiler's ONE resolver rather than by
    // this module spelling a name.
    let entry = plan
        .anchors
        .entry_anchor(&area)
        .expect("the entry place carries the campaign's spawn");
    let delvec::compiler::plan::ResolvedAnchor::Point { pos, .. } = entry else {
        panic!("an entry is a place to stand, not a region");
    };
    assert_eq!(*pos, [7, 64, 11], "the entry stands on the landing's floor");

    // ...and on a DERIVED map it resolves by the declared role (spec-0046), not
    // by the spelling. This pair belongs to neither change on its own: the
    // derivation named its entry `spawn` precisely because the role did not
    // exist yet, and a spelling nobody resolves through is the state that has
    // to be asserted rather than assumed — deleting the role would leave this
    // test green on the fallback and silently reinstate the folklore.
    assert_eq!(
        plan.anchors
            .role_name(&area, delvec::compiler::plan::AnchorRole::Entry),
        Some(delvewright_dsl::ENTRY_ANCHOR),
        "the derivation declares what its entry anchor is FOR"
    );

    // One anchor per place, one gate region per barred way, one unlock anchor
    // for the way that opens from one side only.
    for name in [
        "anchor/node-landing",
        "anchor/node-hall",
        "anchor/node-loft",
        "anchor/node-cell",
        "anchor/node-exit",
        "anchor/seam-hall-cell",
        "anchor/unlock-hall-cell",
    ] {
        assert!(
            plan.anchors.contains_key(&(area.clone(), name.to_string())),
            "the derivation synthesizes `{name}`"
        );
    }
    // The barred seam's region is a GATE, so the world-load seal model measures
    // it exactly as it measures a prefab-authored one.
    let gate = plan
        .anchors
        .get(&(area.clone(), "anchor/seam-hall-cell".to_string()))
        .expect("the barred seam is a gate region");
    assert!(matches!(
        gate,
        delvec::compiler::plan::ResolvedAnchor::Gate { .. }
    ));

    // And the DSL's own answer about which anchors exist agrees with what the
    // derivation placed — one authority, two readers.
    let declared = delvewright_dsl::synthesized_anchors(&c);
    let placed: std::collections::BTreeSet<String> = plan
        .anchors
        .keys()
        .filter(|(a, _)| a == &area)
        .map(|(_, n)| n.clone())
        .collect();
    assert_eq!(
        declared, placed,
        "validation resolves a campaign's anchors against exactly what the derivation places"
    );
}

/// **What every place owes is exactly the synthesized set** (spec-0050 §6;
/// spec-0098 §2 — a seam's gate region is owed by the place owning its plane).
///
/// `dsl::siteplan::synthesized_anchors` is the one authority for which names a
/// site-plan campaign provides, and `owed_anchors` says which of them a given
/// place must re-bind when a piece stands in it. Two functions reading two
/// documents is exactly the drift the one-authority note exists to remove, so
/// this asserts they PARTITION rather than merely overlap.
///
/// Both directions matter and each catches a different defect. A name owed by
/// nobody is a name the campaign resolves and no piece is ever asked for — a
/// quest pointing at a building that does not answer. A name owed by two places
/// is two pieces claiming one anchor, which resolution cannot arbitrate.
#[test]
fn the_owed_anchors_partition_the_synthesized_set() {
    let c = campaign();
    let all = delvewright_dsl::synthesized_anchors(&c);
    assert!(!all.is_empty(), "the fixture provides anchors at all");

    let graph = c.layout_graph.as_ref().map(|g| &g.content).unwrap();
    let mut owed_by: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for n in &graph.nodes {
        for name in delvewright_dsl::owed_anchors(&c, &n.id) {
            owed_by.entry(name).or_default().push(n.id.0.clone());
        }
    }
    for (name, places) in &owed_by {
        assert_eq!(
            places.len(),
            1,
            "`{name}` is owed by {places:?} — two pieces claiming one anchor"
        );
    }

    // Every seam lies in a plane some place owns (spec-0098 §2), so the gate
    // region over a barred way is owed too — by the place that owns its plane.
    let gates: std::collections::BTreeSet<String> = all
        .iter()
        .filter(|n| n.starts_with("anchor/seam-"))
        .cloned()
        .collect();
    assert!(
        !gates.is_empty(),
        "the fixture has a barred way, or this binds nothing"
    );
    let owed: std::collections::BTreeSet<String> = owed_by.keys().cloned().collect();
    assert!(
        gates.is_subset(&owed),
        "every gate region is owed by the place that owns its plane: {:?}",
        gates.difference(&owed).collect::<Vec<_>>()
    );
    assert_eq!(
        owed, all,
        "every synthesized name is owed by exactly one place — there is no second \
         kind, and a name owed by nobody is one no piece is ever asked for"
    );
}

// ---------------------------------------------------------------------------
// A place that is a route, and a hand-off that is not a door (spec-0053)
// ---------------------------------------------------------------------------

/// **The way builds.** `node/tunnel` is a `corridor`: four cells across and
/// eight long, which no rung of the size ladder admits. It derives, assembles
/// and passes the whole battery beside the size-classed places, through the
/// SAME derivation and the same observer — no branch was added for it below the
/// classification itself.
#[test]
fn a_way_classed_box_builds_and_walks_like_any_other_place() {
    let (b, _) = battery_under(Perturb::none());
    assert!(
        errors(&b).is_empty(),
        "the fixture with a way and a contact in it builds clean: {:?}",
        errors(&b)
    );
    assert_eq!(
        b.binding.nodes, 7,
        "every place is proven reached, the way among them"
    );

    // The way is read off the graph rather than asserted: if the fixture stopped
    // declaring one, this says so instead of passing over a campaign that no
    // longer exercises the surface.
    let c = campaign();
    let graph = c.layout_graph.as_ref().expect("the fixture has a graph");
    let ways: Vec<&str> = graph
        .content
        .nodes
        .iter()
        .filter(|n| n.way_class.is_some())
        .map(|n| n.id.0.as_str())
        .collect();
    assert_eq!(
        ways,
        vec!["node/tunnel"],
        "the fixture declares exactly one way, and the battery above proved it"
    );
}

/// **The contact leaves exactly the span open** (spec-0053 acceptance criterion
/// 6): no solid cell inside the span, and wall everywhere outside it on the same
/// shared face at the heights the span occupies.
///
/// Read off the assembled bytes, not off the plan — the plan is what the claim
/// is ABOUT. The two halves are asserted separately because they fail in
/// opposite directions: a derivation that forgot the carve leaves the span
/// solid, and one that carved the whole wall leaves nothing outside it.
#[test]
fn a_contact_leaves_exactly_its_span_open_on_the_shared_wall() {
    use delvewright_dsl::siteplan::Crossing;

    let c = campaign();
    let reg = prefabs();
    let plan = Plan::build_with(&c, &reg, Perturb::none()).expect("the fixture plans");
    let structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let blocks = delvec::compiler::assembled::assembled_blocks(&plan, &structures);
    let bo = plan
        .blockout
        .as_ref()
        .expect("a site-plan campaign has a blockout");

    let contacts: Vec<_> = bo
        .seams
        .iter()
        .filter(|s| s.crossing == Crossing::Contact)
        .collect();
    assert_eq!(contacts.len(), 1, "the fixture allocates one contact");
    let s = contacts[0];
    let (lo, hi) = s.opening;

    let solid_at = |c: [i64; 3]| {
        blocks
            .get(&[c[0] as i32, c[1] as i32, c[2] as i32])
            .is_some_and(|b| b != "minecraft:air")
    };
    let inside = |c: [i64; 3]| (0..3).all(|a| c[a] >= lo[a] && c[a] <= hi[a]);

    // Half one: nothing solid inside the span.
    let mut span_cells = 0usize;
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                span_cells += 1;
                assert!(
                    !solid_at([x, y, z]),
                    "the span is where the derivation writes NO wall, and [{x},{y},{z}] is solid"
                );
            }
        }
    }
    assert!(
        span_cells > 0,
        "the span is not empty, or this binds nothing"
    );

    // Half two: wall everywhere else on the same shared face.
    let (smin, smax) = s.shared;
    let mut outside = 0usize;
    for u in smin[0]..=smax[0] {
        for v in smin[1]..=smax[1] {
            for w in smin[2]..=smax[2] {
                let cell = [u, v, w];
                if inside(cell) {
                    continue;
                }
                outside += 1;
                assert!(
                    solid_at(cell),
                    "wall as ever OUTSIDE the span, and [{u},{v},{w}] is open"
                );
            }
        }
    }
    assert!(
        outside > 0,
        "the shared face is bigger than the span, or half two binds nothing"
    );
}

/// `DW0877`: the massing walls the front the plan allocated, and the observer
/// says nothing crosses it.
///
/// Produced by a **perturbed derivation**, never by hand-authored bytes — the
/// manner spec-0049 §13.8 fixed and spec-0053 §6 asks for by name.
///
/// The knob is the only thing that can produce this red: a portal's cells are
/// untouched, the wall it writes is inside the allocation, and closing a way can
/// only remove crossings. What it can also reach is `DW0837`, and only where the
/// front is the sole walked way into a place — a fact about the fixture, not
/// about the knob. The assertions below are on `DW0877`'s own message, which no
/// other check in this engine produces.
#[test]
fn a_walled_front_reddens_dw0877() {
    let (clean, _) = battery_under(Perturb::none());
    assert!(
        !errors(&clean).contains(&"DW0877".to_string()),
        "the unperturbed derivation leaves the front open: {:?}",
        errors(&clean)
    );
    assert!(
        clean.binding.contact_columns > 0,
        "and it measures a non-zero crossing profile, or the red below proves nothing"
    );

    let (b, _) = battery_under(Perturb {
        wall_contacts: true,
        ..Perturb::none()
    });
    assert!(
        errors(&b).contains(&"DW0877".to_string()),
        "a walled front is a hand-off the graph declares and the world does not have: {:?}",
        errors(&b)
    );
    let m = message_for(&b, "DW0877");
    assert!(
        m.contains("nothing crosses the contact"),
        "the refusal names what it measured: {m}"
    );
    assert!(
        m.contains("longest unbroken run"),
        "and states the run against what a body needs: {m}"
    );
    assert_eq!(
        b.binding.contacts, 1,
        "the binding states the denominator even when the check refuses"
    );
    assert_eq!(
        b.binding.contact_columns, 0,
        "and the numerator, which is what went to zero"
    );
}

/// `DW0986`: a barred door whose far side the massing walls flush behind its
/// opening — the shape a stair's treads laid across a doorway take — and the
/// observer says nothing crosses it.
///
/// Produced by a **perturbed derivation**, never by hand-authored bytes. The
/// knob is the only thing that can produce this red: the opening's cells stay
/// clear (`DW0836`'s claim 1), the wall it writes is off the shared wall's plane
/// (claim 2), and closing a way only removes crossings (`DW0838`). The
/// assertions are on `DW0986`'s own message, which no other check produces.
#[test]
fn a_barred_door_onto_a_wall_reddens_dw0986() {
    let (clean, _) = battery_under(Perturb::none());
    assert!(
        !errors(&clean).contains(&"DW0986".to_string()),
        "the unperturbed derivation's portals all lead through: {:?}",
        errors(&clean)
    );
    let portals = clean.binding.seams - clean.binding.contacts;
    assert_eq!(
        clean.binding.portals + clean.binding.portals_solid,
        portals,
        "every portal is either measured or left to `DW0836`, and none is lost"
    );
    assert_eq!(
        clean.binding.portals_solid, 0,
        "no opening is solid when nothing is perturbed"
    );
    assert!(
        clean.binding.portal_floor > 0,
        "the crossing stood a body in some opening, or the red below proves nothing"
    );

    let (b, _) = battery_under(Perturb {
        bury_barred: true,
        ..Perturb::none()
    });
    assert!(
        errors(&b).contains(&"DW0986".to_string()),
        "a bar that opens onto a wall is a way the graph declares and no body can take: {:?}",
        errors(&b)
    );
    assert!(
        !errors(&b).contains(&"DW0836".to_string()) && !errors(&b).contains(&"DW0838".to_string()),
        "the buried door is a defect only the crossing can see: {:?}",
        errors(&b)
    );
    let m = message_for(&b, "DW0986");
    assert!(
        m.contains("nothing crosses the barred opening"),
        "the refusal names the seam's class and what it measured: {m}"
    );
    assert!(
        m.contains("must step onto standable ground on BOTH sides"),
        "and states the quantifier it holds the opening to: {m}"
    );
    assert!(
        m.contains("move this seam along its face (`at`)"),
        "and names the remedy: {m}"
    );
    assert_eq!(
        b.binding.portals, clean.binding.portals,
        "the binding states the denominator even when the check refuses"
    );
}

/// The fixture with its hall's stair laid flush along the barred wall, and the
/// barred door at `at` along that wall — the shape The Stranding's spine took,
/// where a doorway opened onto the side of a stair's upper flight.
///
/// The hall is made eight deep and its stair moved to the south wall
/// (`edge/hall-loft` at 6), with every seam that hangs the loft and the landing
/// re-anchored so each box keeps a legal place (`meets`, the loft's contact,
/// the sightline's far end). Only `edge/hall-cell`'s `at` differs between the
/// two worlds the remedy test builds.
fn door_along_the_stair(at: i64) -> Campaign {
    let loaded = delvec::compiler::load::load_campaign_dir(&fixture_dir())
        .expect("the blockout fixture is readable");
    let mut raw = loaded.raw;
    let mut plan: serde_json::Value = serde_json::from_str(
        raw.site_plan
            .as_deref()
            .expect("the blockout fixture carries a site plan"),
    )
    .expect("the site plan is JSON");
    let content = &mut plan["content"];
    let mut edited = 0;
    for b in content["boxes"].as_array_mut().expect("boxes") {
        if b["node"] == "node/hall" {
            b["extent"] = serde_json::json!([12, 8]);
            edited += 1;
        }
    }
    for s in content["seams"].as_array_mut().expect("seams") {
        match s["edge"].as_str().expect("an edge id") {
            "edge/landing-hall" => s["meets"] = serde_json::json!(0),
            "edge/hall-loft" => {
                s["at"] = serde_json::json!(6);
                s["meets"] = serde_json::json!(4);
            }
            "edge/loft-drop" => s["meets"] = serde_json::json!(2),
            "edge/hall-cell" => s["at"] = serde_json::json!(at),
            _ => continue,
        }
        edited += 1;
    }
    let to = &mut content["sightlines"][0]["to"][2];
    *to = serde_json::json!(to.as_i64().expect("a sightline end") + 4);
    assert_eq!(edited, 5, "every edit this shape names found its object");
    raw.site_plan = Some(plan.to_string());
    delvewright_dsl::parse_campaign(&raw).expect("the edited fixture parses")
}

fn battery_of(c: &Campaign) -> blockout::Battery {
    let reg = prefabs();
    let plan = Plan::build_with(c, &reg, Perturb::none()).expect("the edited fixture plans");
    let structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let blocks = delvec::compiler::assembled::assembled_blocks(&plan, &structures);
    blockout::check(&plan, &blocks).expect("a site-plan campaign has a blockout")
}

/// `DW0986`'s remedy is reachable: the refusal says *move this seam along its
/// face (`at`)*, and taking exactly that move turns the verdict.
///
/// At `at` 7 the barred door opens onto the stair's courses three and four high,
/// so a body in it steps back into the cell and nowhere else. Here the door is
/// the cell's only way in, so `DW0837` sees the cell unreached as well; `DW0986`
/// is the refusal that names the door. At `at` 2 the door meets the stair's
/// lowest course and the floor beside it, and the whole battery is green.
#[test]
fn moving_a_door_off_the_treads_is_the_remedy_dw0986_names() {
    let red = battery_of(&door_along_the_stair(7));
    assert!(
        errors(&red).contains(&"DW0986".to_string()),
        "a door onto the side of a flight is a way no body takes: {:?}",
        errors(&red)
    );
    let m = message_for(&red, "DW0986");
    assert!(
        m.contains("`edge/hall-cell`"),
        "the refusal names the door: {m}"
    );
    assert!(
        m.contains("0 step into `node/hall`"),
        "and says which side has no floor: {m}"
    );

    let green = battery_of(&door_along_the_stair(2));
    assert!(
        errors(&green).is_empty(),
        "the move the refusal prescribes reaches a different verdict: {:?}",
        errors(&green)
    );
    assert_eq!(
        green.binding.portals, 6,
        "six portals measured, the door among them"
    );
}

// ---------------------------------------------------------------------------
// spec-0098: a place owns its outside — the claim, the fill, the terrain
// ---------------------------------------------------------------------------

use delvewright_dsl::Diagnostic;
use delvewright_dsl::siteplan::{Owner, SitePlan};

/// A scratch copy of the blockout fixture, its site plan edited by `edit`, and
/// with a heightmap image written beside it when `heightmap` is given.
fn variant(
    tag: &str,
    edit: impl FnOnce(&mut serde_json::Value),
    heightmap: Option<&dyn Fn(i64, i64) -> i64>,
) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-place-shell-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&fixture_dir(), &dir);
    if let Some(h) = heightmap {
        let img = image::GrayImage::from_fn(64, 64, |x, z| {
            image::Luma([((h(i64::from(x), i64::from(z)) - 56) * 17) as u8])
        });
        let mut bytes = std::io::Cursor::new(Vec::new());
        img.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        std::fs::write(dir.join("terrain.png"), bytes.into_inner()).unwrap();
        common::patch_file(&dir.join("site-plan.json"), |v| {
            v["content"]["fill"]["terrain"] = serde_json::json!({
                "kind": "heightmap", "heightmap": "terrain.png", "base_y": 56, "range": 15
            });
        });
    }
    common::patch_file(&dir.join("site-plan.json"), edit);
    dir
}

/// The sloped terrain the stitch tests stand on: the landing's side of the map
/// falls to between three and five courses under the floor course, in steps
/// along z, and the rest stands at the floor course.
fn slope(x: i64, z: i64) -> i64 {
    if x <= 12 { 59 + (z / 4) % 3 } else { 63 }
}

/// The final block map a derivation writes, replayed in order.
fn mass_map(c: &Campaign) -> BTreeMap<[i64; 3], String> {
    let mut reads = delvewright_dsl::metrics::Reads::new();
    let (placement, _) = blockout::derive(c, &mut reads).expect("a site-plan campaign derives");
    let mut out = BTreeMap::new();
    for f in &placement.mass {
        let lo = [
            i64::from(f.from[0]),
            i64::from(f.from[1]),
            i64::from(f.from[2]),
        ];
        let hi = [i64::from(f.to[0]), i64::from(f.to[1]), i64::from(f.to[2])];
        for cell in blockout::cells_of(lo, hi) {
            if f.block == "minecraft:air" {
                out.remove(&cell);
            } else {
                out.insert(cell, f.block.clone());
            }
        }
    }
    out
}

fn with_bound(c: &Campaign, nodes: &[&str]) -> Campaign {
    let mut c = c.clone();
    let rows: Vec<serde_json::Value> = nodes
        .iter()
        .map(|n| serde_json::json!({"place": n, "piece": "prefab/anything"}))
        .collect();
    let doc = serde_json::json!({
        "campaign_id": "blockout",
        "content": {"details": rows},
        "dsl_version": delvewright_dsl::DSL_VERSION,
        "stage": "detail-plan",
    });
    c.detail_plan = Some(serde_json::from_value(doc).expect("a detail plan parses"));
    c
}

/// What the whole holds at a cell no place claims: the last volume over it,
/// else the fill.
fn whole_block(
    c: &Campaign,
    ground: &delvewright_dsl::siteplan::Ground,
    cell: [i64; 3],
) -> Option<String> {
    let plan = &c.site_plan.as_ref().unwrap().content;
    let mut out = ground.fill_block(cell).map(str::to_string);
    for v in &plan.volumes {
        let (lo, hi) = (v.region.min, v.region.max());
        if (0..3).all(|i| cell[i] >= lo[i] && cell[i] <= hi[i]) {
            out = match (&v.block, v.role) {
                (_, delvewright_dsl::siteplan::VolumeRole::Clearance) => None,
                (Some(b), _) => Some(b.clone()),
                (None, role) => ground.volume_block(role, cell, hi[1]).map(str::to_string),
            };
        }
    }
    out
}

/// **Criterion 3: the derivation writes only stand-ins, only in unbound
/// claims.** With one place bound, no cell that place owns is written at all,
/// every cell no place claims holds exactly what the fill and the volumes
/// declare, and the ring's fixed ground holds the terrain's block; with
/// nothing bound, the same holds of every unclaimed and fixed cell. Vacuous if
/// the bound place owns nothing or the fixture has no gap — both counted.
#[test]
fn the_derivation_writes_only_stand_ins_and_only_in_unbound_claims() {
    let c = campaign();
    let plan = SitePlan::of(&c);
    let site = plan.site();
    let region = c.site_plan.as_ref().unwrap().content.region;
    let (rlo, rhi) = (region.min, region.max());
    for bound in [&["node/hall"][..], &[][..]] {
        let map = mass_map(&with_bound(&c, bound));
        let (mut bound_cells, mut gaps, mut fixed) = (0usize, 0usize, 0usize);
        for cell in blockout::cells_of(rlo, rhi) {
            match site.owner(cell) {
                Owner::Place(n) if bound.contains(&n.0.as_str()) => {
                    bound_cells += 1;
                    assert!(
                        !map.contains_key(&cell),
                        "the derivation wrote {:?} at {cell:?}, which the bound `{n}` owns",
                        map.get(&cell)
                    );
                }
                Owner::Nobody => {
                    gaps += 1;
                    assert_eq!(
                        map.get(&cell),
                        whole_block(&c, &plan.ground, cell).as_ref(),
                        "a cell no place claims holds what the plan declares, at {cell:?}"
                    );
                }
                Owner::Ground => {
                    fixed += 1;
                    let i = (0..plan.boxes.len())
                        .find(|&i| site.is_fixed(i, cell))
                        .unwrap();
                    let g = site.ground_height(i, cell[0], cell[2]);
                    assert_eq!(
                        map.get(&cell).map(String::as_str),
                        Some(plan.ground.ground_block(cell, g)),
                        "the ring's fixed ground is the terrain, at {cell:?}"
                    );
                }
                Owner::Contested(ns) => panic!("the fixture contests {cell:?} between {ns:?}"),
                Owner::Place(_) => {}
            }
        }
        assert!(
            gaps > 0 && fixed > 0,
            "the fixture has gaps ({gaps}) and fixed ground ({fixed})"
        );
        if !bound.is_empty() {
            assert!(
                bound_cells > 0,
                "the bound place owns {bound_cells} cell(s)"
            );
        }
    }
}

/// **Criterion 19: `fill` is required, and both kinds derive.** The fixture
/// derives under `solid` and under `open` with the battery green, and the two
/// derivations agree at every cell a place owns under both and disagree at
/// some cell no place owns.
#[test]
fn both_fills_derive_and_differ_only_where_no_place_owns() {
    let open = campaign();
    let solid_dir = variant(
        "solid",
        |v| {
            v["content"]["fill"] =
                serde_json::json!({"kind": "solid", "block": "minecraft:deepslate"});
        },
        None,
    );
    let solid = common::campaign_at(&solid_dir);
    for c in [&open, &solid] {
        let b = battery_of(c);
        assert!(errors(&b).is_empty(), "{:?}", errors(&b));
    }
    let (po, ps) = (SitePlan::of(&open), SitePlan::of(&solid));
    let (so, ss) = (po.site(), ps.site());
    let (mo, ms) = (mass_map(&open), mass_map(&solid));
    let region = open.site_plan.as_ref().unwrap().content.region;
    let (mut same, mut differ) = (0usize, 0usize);
    for cell in blockout::cells_of(region.min, region.max()) {
        let (a, b) = (so.owner(cell), ss.owner(cell));
        match (&a, &b) {
            (Owner::Place(x), Owner::Place(y)) if x == y => {
                same += 1;
                assert_eq!(
                    mo.get(&cell),
                    ms.get(&cell),
                    "a cell `{x}` owns under both, {cell:?}"
                );
            }
            (Owner::Nobody, Owner::Nobody) => {
                if mo.get(&cell) != ms.get(&cell) {
                    differ += 1;
                }
            }
            _ => {}
        }
    }
    assert!(same > 0, "the comparison saw no owned cell");
    assert!(
        differ > 0,
        "the two fills disagree nowhere a place does not own"
    );
}

/// **Criterion 20: the terrain is declared and placed.** A heightmap the
/// wrong size is refused naming both sizes, one standing over the region's
/// top is `DW0826`, and the sloped one derives a surface whose top at every
/// unclaimed column is the image's value — moving exactly that column when one
/// pixel changes.
#[test]
fn the_terrain_is_the_heightmap_column_for_column() {
    let refusals = |c: &Campaign| -> Vec<Diagnostic> {
        let mut reads = delvewright_dsl::metrics::Reads::new();
        let mut d = Vec::new();
        delvewright_dsl::siteplan::check(c, &mut reads, &mut d);
        d
    };
    // The wrong size.
    let dir = variant("hm-size", |_| {}, Some(&slope));
    let img = image::GrayImage::from_pixel(32, 64, image::Luma([100]));
    img.save(dir.join("terrain.png")).unwrap();
    let d = refusals(&common::campaign_at(&dir));
    let m = d.iter().find(|x| x.code == "DW0826").unwrap_or_else(|| {
        panic!(
            "no DW0826: {:?}",
            d.iter().map(|x| &x.code).collect::<Vec<_>>()
        )
    });
    assert!(
        m.message.contains("32 × 64") && m.message.contains("64 × 64"),
        "{}",
        m.message
    );
    // Over the region's top (y 87): base 90, range 15.
    let dir = variant(
        "hm-high",
        |v| v["content"]["fill"]["terrain"]["base_y"] = serde_json::json!(90),
        Some(&slope),
    );
    let d = refusals(&common::campaign_at(&dir));
    assert!(
        d.iter()
            .any(|x| x.code == "DW0826" && x.message.contains("terrain leaves the region")),
        "{:?}",
        d.iter().map(|x| &x.message).collect::<Vec<_>>()
    );
    // Column for column.
    let dir = variant("hm-slope", |_| {}, Some(&slope));
    let c = common::campaign_at(&dir);
    let plan = SitePlan::of(&c);
    let site = plan.site();
    let map = mass_map(&c);
    let surface_top = |map: &BTreeMap<[i64; 3], String>, x: i64, z: i64| {
        (56..=87)
            .rev()
            .find(|y| map.get(&[x, *y, z]).map(String::as_str) == Some("minecraft:grass_block"))
    };
    let mut columns = 0usize;
    for x in 0..64 {
        for z in 0..64 {
            let free = (56..=87).all(|y| site.owner([x, y, z]) == Owner::Nobody);
            if !free {
                continue;
            }
            columns += 1;
            assert_eq!(
                surface_top(&map, x, z),
                Some(slope(x, z)),
                "column [{x}, {z}]"
            );
        }
    }
    assert!(columns > 1000, "{columns} unclaimed column(s) compared");
    // One pixel moved: exactly that column moves.
    let moved = |x: i64, z: i64| {
        if (x, z) == (60, 60) {
            slope(x, z) + 2
        } else {
            slope(x, z)
        }
    };
    let dir2 = variant("hm-pixel", |_| {}, Some(&moved));
    let map2 = mass_map(&common::campaign_at(&dir2));
    let changed: std::collections::BTreeSet<[i64; 2]> = map
        .keys()
        .chain(map2.keys())
        .filter(|k| map.get(*k) != map2.get(*k))
        .map(|k| [k[0], k[2]])
        .collect();
    assert_eq!(
        changed.into_iter().collect::<Vec<_>>(),
        vec![[60, 60]],
        "one pixel moves exactly its own column"
    );
}

/// **Criterion 21's perturbation: `Perturb::hollow_edge` reds `DW0990` alone.**
/// On the slope it opens air under the landing's edge, and `DW0990` — and
/// nothing else — refuses; the same variant derived without it is green.
#[test]
fn a_hollow_edge_reddens_dw0990_alone() {
    let hollow = Perturb {
        hollow_edge: true,
        ..Perturb::none()
    };
    let dir = variant("hollow", |_| {}, Some(&slope));
    let c = common::campaign_at(&dir);
    let clean = battery_of_with(&c, Perturb::none());
    assert!(errors(&clean).is_empty(), "{:?}", errors(&clean));
    assert!(clean.binding.boundary_columns > 0 && clean.binding.cracks == 0);
    let b = battery_of_with(&c, hollow);
    assert_eq!(
        errors(&b)
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        ["DW0990".to_string()].into_iter().collect(),
        "the hollow edge is a crack and nothing else"
    );
    let m = message_for(&b, "DW0990");
    assert!(
        m.contains("node/landing") && m.contains("stands at y"),
        "{m}"
    );
    assert!(b.binding.cracks > 0);
}

fn battery_of_with(c: &Campaign, perturb: Perturb) -> blockout::Battery {
    let reg = prefabs();
    let plan = Plan::build_with(c, &reg, perturb).expect("the variant plans");
    let blocks = delvec::compiler::assembled::assembled_blocks(&plan, &BTreeMap::new());
    blockout::check(&plan, &blocks).expect("a site-plan campaign has a blockout")
}

/// **Criterion 23, the derivation's half: the ring is fixed.** Over the slope,
/// every fixed ring cell holds the terrain's block with nothing bound; at the
/// landing's door the ring's ground is the sill's course, flat across the
/// opening; and the hall's stair treads lie inside its play space. Vacuous if
/// the terrain were flat at the floor: the plot's terrain range is asserted.
#[test]
fn the_ring_is_the_terrain_and_a_door_stands_on_its_sill() {
    let dir = variant("ring", |_| {}, Some(&slope));
    let c = common::campaign_at(&dir);
    let plan = SitePlan::of(&c);
    let site = plan.site();
    let map = mass_map(&c);
    let landing = site
        .index_of(&delvewright_dsl::NodeId("node/landing".into()))
        .unwrap();
    let b = &plan.boxes[landing];
    let tops: Vec<i64> = (b.foot[0] - 1..=b.foot[1] + 1)
        .flat_map(|x| (b.foot[2] - 1..=b.foot[3] + 1).map(move |z| (x, z)))
        .filter(|(x, z)| b.is_ring_column(*x, *z))
        .map(|(x, z)| plan.ground.top(x, z).unwrap())
        .collect();
    let (lo, hi) = (tops.iter().min().unwrap(), tops.iter().max().unwrap());
    assert!(hi - lo > 1, "the plot's terrain spans {lo}..{hi}");
    let mut examined = 0usize;
    for i in 0..plan.boxes.len() {
        for (cell, g) in site.fixed_cells(i) {
            examined += 1;
            assert_eq!(
                map.get(&cell).map(String::as_str),
                Some(plan.ground.ground_block(cell, g)),
                "fixed cell {cell:?}"
            );
        }
    }
    assert!(examined > 0);
    // The landing's door to the hall: its columns' ground is the sill minus one.
    let door = plan
        .seams
        .iter()
        .find(|s| s.edge.0 == "edge/landing-hall")
        .unwrap();
    for z in door.opening.0[2]..=door.opening.1[2] {
        assert_eq!(
            site.ground_height(landing, door.plane, z),
            door.opening.0[1] - 1
        );
        assert_eq!(
            map.get(&[door.plane, door.opening.0[1] - 1, z])
                .map(String::as_str),
            Some("minecraft:grass_block"),
            "the sill stands on the ring's ground at z {z}"
        );
    }
    // The hall's stair: every tread inside its play space.
    let hall = &plan.boxes[site
        .index_of(&delvewright_dsl::NodeId("node/hall".into()))
        .unwrap()];
    let (slo, shi) = hall.space();
    let treads: Vec<&[i64; 3]> = map
        .iter()
        .filter(|(_, b)| b.starts_with("minecraft:polished_diorite"))
        .map(|(c, _)| c)
        .collect();
    assert!(!treads.is_empty());
    for t in treads
        .iter()
        .filter(|t| (0..3).all(|i| t[i] >= slo[i] - 1 && t[i] <= shi[i] + 1))
    {
        assert!(
            (0..3).all(|i| t[i] >= slo[i] && t[i] <= shi[i]),
            "a tread at {t:?} stands outside the hall's play space"
        );
    }
}

/// The gallery's site-plan overlay point, materialised the way
/// `tools/ci/gallery_domain.py` does: the primary minus its non-campaign
/// directories, the overlay's files laid over it.
fn gallery_site_plan() -> Campaign {
    let dir = std::env::temp_dir().join("dw-place-shell-gallery-site-plan");
    let _ = std::fs::remove_dir_all(&dir);
    let gallery = common::repo_root().join("gallery");
    common::copy_dir_all(&gallery, &dir);
    for junk in ["baseline", "forms", "overlays", "probes"] {
        let _ = std::fs::remove_dir_all(dir.join(junk));
    }
    common::copy_dir_all(&gallery.join("overlays/site-plan"), &dir);
    let _ = std::fs::remove_file(dir.join("overlay.json"));
    common::campaign_at(&dir)
}

/// **Criterion 1: the ownership rule is exhaustive and one-owner.** Over the
/// blockout fixture, the gallery's site-plan overlay and a hand-built stacked
/// pair, the owned-cell sets of distinct places are disjoint and every claimed
/// cell is owned by exactly one of them, by the ring's fixed ground, or is
/// contested (none is, on a plan that validates); the enumerated count equals
/// the union of the claims. Each of rules 3a–3c is reached by a named cell on
/// these plans, and 3d by the hand-built pair.
#[test]
fn the_ownership_rule_is_exhaustive_and_one_owner() {
    let check = |label: &str, plan: &SitePlan| -> usize {
        let site = plan.site();
        let mut union: std::collections::BTreeSet<[i64; 3]> = std::collections::BTreeSet::new();
        for i in 0..plan.boxes.len() {
            union.extend(site.claim_cells(i));
        }
        let mut owned: BTreeMap<[i64; 3], String> = BTreeMap::new();
        let mut counted = 0usize;
        for i in 0..plan.boxes.len() {
            let o = site.ownership(i);
            for (lo, hi) in &o.owned {
                for c in blockout::cells_of(*lo, *hi) {
                    counted += 1;
                    let prev = owned.insert(c, plan.boxes[i].node.0.clone());
                    assert!(
                        prev.is_none(),
                        "{label}: {c:?} owned by {prev:?} and {}",
                        plan.boxes[i].node
                    );
                }
            }
        }
        let mut enumerated = 0usize;
        for c in &union {
            enumerated += 1;
            match site.owner(*c) {
                Owner::Place(n) => assert_eq!(owned.get(c), Some(&n.0), "{label}: {c:?}"),
                Owner::Ground => assert!(!owned.contains_key(c)),
                other => panic!("{label}: {c:?} is {other:?}"),
            }
        }
        assert_eq!(enumerated, union.len(), "{label}");
        assert_eq!(counted, owned.len(), "{label}");
        assert!(
            counted > 0 && counted <= union.len(),
            "{label}: {counted} of {}",
            union.len()
        );
        union.len()
    };
    let bo = SitePlan::of(&campaign());
    assert!(check("blockout", &bo) > 1000);
    let gallery = gallery_site_plan();
    let gp = SitePlan::of(&gallery);
    assert!(check("gallery", &gp) > 1000);

    // 3a — a stacked floor: the cell over the undercroft is the cell's floor
    // course, which is also the undercroft's lid.
    let site = bo.site();
    let by = |n: &str| &bo.boxes[site.index_of(&delvewright_dsl::NodeId(n.into())).unwrap()];
    let cell = by("node/cell");
    let under = by("node/undercroft");
    let plane = [under.foot[0] + 1, cell.floor - 1, under.foot[2] + 1];
    assert_eq!(
        under.top() + 1,
        plane[1],
        "the undercroft's lid is that plane"
    );
    assert_eq!(site.owner(plane), Owner::Place(cell.node.clone()), "3a");
    // 3b — a facade onto an open place: the hall's wall beside the open loft,
    // off every seam, above the loft's floor course.
    let (hall, loft) = (by("node/hall"), by("node/loft"));
    let facade = [hall.foot[1] + 1, loft.floor + 2, loft.foot[3]];
    assert!(site.claims(site.index_of(&loft.node).unwrap(), facade));
    assert_eq!(site.owner(facade), Owner::Place(hall.node.clone()), "3b");
    // 3c — between two roofed places a seam joins: the landing (its `a`) draws
    // the party wall with the hall.
    let landing = by("node/landing");
    let party = [landing.foot[1] + 1, landing.floor + 3, landing.foot[2]];
    assert!(site.claims(site.index_of(&hall.node).unwrap(), party));
    assert_eq!(site.owner(party), Owner::Place(landing.node.clone()), "3c");
    // 3d — two roofed places one apart with nothing joining them.
    let g = delvewright_dsl::siteplan::Ground::solid("minecraft:stone");
    let pair = vec![
        delvewright_dsl::siteplan::PlacedBox {
            node: delvewright_dsl::NodeId("node/a".into()),
            foot: [0, 7, 0, 7],
            floor: 64,
            clearance: 4,
            open: false,
            roof: None,
        },
        delvewright_dsl::siteplan::PlacedBox {
            node: delvewright_dsl::NodeId("node/b".into()),
            foot: [9, 16, 0, 7],
            floor: 64,
            clearance: 4,
            open: false,
            roof: None,
        },
    ];
    let site = delvewright_dsl::siteplan::Site::new(&pair, &[], &g);
    assert!(matches!(site.owner([8, 65, 3]), Owner::Contested(_)), "3d");
}

/// **Criterion 16, at the campaign: `DW0827` refuses two places one cell
/// apart with no connection.** Hanging the exit four cells further north off
/// the cell's west face stands it one cell from the landing, which nothing
/// joins it to; the refusal names both. The fixture as written — the same two
/// places three cells apart — is green (the remedies at the rule are proven in
/// `crates/dsl`'s claim tests).
#[test]
fn dw0827_refuses_two_places_one_cell_apart_with_nothing_joining_them() {
    let dir = variant(
        "one-apart",
        |v| {
            for s in v["content"]["seams"].as_array_mut().unwrap() {
                if s["edge"] == "edge/cell-exit" {
                    s["meets"] = serde_json::json!(6);
                }
            }
        },
        None,
    );
    let c = common::campaign_at(&dir);
    let mut reads = delvewright_dsl::metrics::Reads::new();
    let mut d = Vec::new();
    delvewright_dsl::siteplan::check(&c, &mut reads, &mut d);
    let e = d
        .iter()
        .find(|x| {
            x.code == "DW0827"
                && x.message.contains("node/landing")
                && x.message.contains("node/exit")
        })
        .unwrap_or_else(|| panic!("{:?}", d.iter().map(|x| &x.message).collect::<Vec<_>>()));
    assert!(e.message.contains("no rule awards them"), "{}", e.message);

    let mut d = Vec::new();
    delvewright_dsl::siteplan::check(&campaign(), &mut reads, &mut d);
    assert!(
        !d.iter().any(|x| x.code == "DW0827"),
        "the fixture as written"
    );
}
