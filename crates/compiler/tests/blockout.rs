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

use delvewright_compiler::blockout::{self, Perturb};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::siteplan::PlacedBox;
use delvewright_dsl::{Campaign, Severity};

/// The site-plan fixture: five places, zero authored geometry.
fn fixture_dir() -> std::path::PathBuf {
    common::repo_root().join("crates/compiler/tests/fixtures/blockout")
}

fn campaign() -> Campaign {
    let loaded = delvewright_compiler::load::load_campaign_dir(&fixture_dir())
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
    let blocks = delvewright_compiler::assembled::assembled_blocks(&plan, &structures);
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
    assert_eq!(k.seams, 6, "six traversal connections are allocated");
    assert_eq!(
        k.walls, 5,
        "six seams over five walls — the stair and the fall pierce one wall, and \
         the undercroft's own stair pierces the cell's floor"
    );
    assert_eq!(k.nodes, 6, "six places are proven reached");
    assert!(
        k.standable > 500,
        "the crossing check classified {} standable cell(s), which is not a map",
        k.standable
    );
    assert_eq!(k.pairs, 15, "six places make fifteen unordered pairs");
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
    assert_eq!(k.boxes, 6);
    assert_eq!(k.seams, 6);
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
        b.binding.seams, 6,
        "the binding is stated even when the check refuses"
    );
}

/// `DW0836`'s other half: the mass is laid at a height the plan did not choose,
/// and nothing at stage 4 could ever have seen it.
#[test]
fn a_sunk_place_reddens_dw0836_on_the_realized_rise() {
    let (b, _) = battery_under(Perturb {
        sink: Some("node/loft"),
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
        brick_up: Some("node/exit"),
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
    assert_eq!(b.binding.nodes, 6, "all six places were examined");
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
    let m = message_for(&b, "DW0838");
    assert!(
        m.contains("allocated no seam for") && m.contains("can still walk to"),
        "the refusal names both places and a witness cell: {m}"
    );
    assert_eq!(b.binding.pairs, 15);
}

// ---------------------------------------------------------------------------
// The headroom measure, and the run a stair really has
// ---------------------------------------------------------------------------

/// The derived world of the unperturbed fixture, for tests that read blocks
/// rather than findings.
fn derived_world() -> (Vec<PlacedBox>, delvewright_compiler::nav::World) {
    let c = campaign();
    let reg = prefabs();
    let plan = Plan::build(&c, &reg).expect("the blockout fixture plans");
    let structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let world = delvewright_compiler::nav::World::from_plan(&plan, &structures);
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
        low_ceiling: Some("node/landing"),
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
        let found = (lo[2]..=hi[2])
            .any(|z| (lo[0]..=hi[0]).any(|x| world.is_standable(cell([x, y, z]))));
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
/// only constructor anything outside a test reaches — asks for no defect, and a
/// build made through it is byte-identical to one made by naming
/// [`Perturb::none`] explicitly.
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

    // The entry, resolved through the compiler's own alias list rather than by
    // this module spelling a name.
    let entry = delvewright_compiler::plan::entry_anchor(&plan.anchors, &area)
        .expect("the entry place carries the campaign's spawn");
    let delvewright_compiler::plan::ResolvedAnchor::Point { pos, .. } = entry else {
        panic!("an entry is a place to stand, not a region");
    };
    assert_eq!(*pos, [7, 64, 11], "the entry stands on the landing's floor");

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
        delvewright_compiler::plan::ResolvedAnchor::Gate { .. }
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

/// **What every place owes, plus the gate regions no place owes, is exactly the
/// synthesized set** (spec-0050 §6).
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

    // The gate regions are the one family no place owes: they stand in a party
    // plane the whole owns, not inside any piece.
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
        owed.is_disjoint(&gates),
        "a gate region is never owed by a place"
    );
    let union: std::collections::BTreeSet<String> = owed.union(&gates).cloned().collect();
    assert_eq!(
        union, all,
        "every synthesized name is either owed by exactly one place or a gate \
         region the whole keeps — there is no third kind, and a name in neither \
         is one no piece is ever asked for"
    );
}
