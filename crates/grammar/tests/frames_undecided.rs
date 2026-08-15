//! **The third answer** (`DW0742`): a region that cannot decide the
//! `oriented-fills` gate.
//!
//! `DW0736` asks whether a world-frame literal survives the frame its scope was
//! given, and its first act is to return "sound" for the identity frame. That is
//! right as far as it goes, and it is the whole defect: a scope reoriented onto
//! the identity *by this region's proportions* is one the predicate never read,
//! and the same program at a region whose axes rank differently hands that scope
//! a quarter-turn and refuses the same literal outright. One program, two
//! regions, two opposite verdicts, and nothing in the green one said so.
//!
//! Every test here is one half of that pair. The pass/fail axis is the same
//! program at two regions; the noise axis is the four ways a fill is genuinely
//! decided — no reorientation at all, a reorientation that names its axes
//! outright, a state the reachable frames cannot disturb, and a state written in
//! the scope's own frame — none of which may report anything.

use delvewright_grammar::block::BlockState;
use delvewright_grammar::export::export_zone;
use delvewright_grammar::gates::{self, GateState};
use delvewright_grammar::ir::{AxisSpec, Material, Node, Paint, Program, Reorient};
use delvewright_grammar::{Box3, ExpandOptions, expand};

/// Longest along Z, so `z: largest` is the identity here.
const DECLARED: [u32; 3] = [5, 4, 9];
/// The same box transposed: longest along X, so `z: largest` is a quarter turn.
const TRANSPOSED: [u32; 3] = [9, 4, 5];

/// A run of bars across the local X of the scope, written in WORLD directions —
/// the shape the live campaign's gate ward ships.
const BARS: &str =
    "minecraft:iron_bars[east=true,north=false,south=false,waterlogged=false,west=true]";
/// A pillar along the vertical. Frame-sensitive in general — some frame lays it
/// on its side — but not under a request that pins the vertical.
const PILLAR: &str = "minecraft:deepslate[axis=y]";

fn state(s: &str) -> BlockState {
    s.parse().expect("a legal block state")
}

/// `<reorient> { fill <paint> }`, or the bare fill when there is no request.
fn program(request: Option<Reorient>, paint: Paint) -> Program {
    let fill = Node::Fill {
        material: Material::Inline(paint),
    };
    Program::new("undecided", "root").rule(
        "root",
        match request {
            Some(orient) => Node::Reorient {
                orient,
                body: Box::new(fill),
            },
            None => fill,
        },
    )
}

/// The request the campaign zones use: keep the vertical on the world's, and
/// let the long axis of the box decide which horizontal is "along".
fn largest_z() -> Reorient {
    Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest)
}

fn judge(program: &Program, region: [u32; 3]) -> gates::Report {
    let out = expand(program, Box3::at_origin(region), &ExpandOptions::seeded(1))
        .expect("the program expands");
    gates::judge(&out, gates::Options::default())
}

fn oriented(report: &gates::Report) -> &gates::Gate {
    report
        .gates
        .iter()
        .find(|g| g.id == "oriented-fills")
        .expect("every report carries the oriented-fills gate")
}

/// **The pair.** One program, two regions. At the declared one the scope's
/// frame is the identity and `DW0736` reads nothing; at the transposed one the
/// same scope is turned and the same literal is refused. Before the third
/// answer existed the first of these was a `pass`.
#[test]
fn the_same_bare_fill_is_undecided_at_one_region_and_refused_at_its_transpose() {
    let p = program(Some(largest_z()), Paint::block(state(BARS)));

    let here = judge(&p, DECLARED);
    let g = oriented(&here);
    assert_eq!(g.state, GateState::Undecided, "{}", g.detail);
    assert!(!g.passed(), "an undecided gate is not a pass");
    assert!(!g.failed(), "and it is not a fail either");
    assert_eq!(g.undecided, 1, "{}", g.detail);
    assert!(g.bound > 0, "{}", g.detail);
    assert!(g.detail.contains("DW0742"), "{}", g.detail);
    assert!(g.detail.contains("east=true"), "{}", g.detail);
    // The reorientation request is named, not described: it is the thing to go
    // and read, and a message that only said "a reorientation" would send an
    // author looking through the whole program for it.
    assert!(g.detail.contains("z: largest"), "{}", g.detail);
    assert_eq!(here.verdict, "undecided");
    assert!(!here.is_pass() && !here.is_fail() && here.is_undecided());
    assert!(
        here.findings.iter().any(|f| f.contains("could not decide")),
        "{:?}",
        here.findings
    );

    let there = judge(&p, TRANSPOSED);
    let g = oriented(&there);
    assert!(g.failed(), "{}", g.detail);
    assert!(g.detail.contains("DW0736"), "{}", g.detail);
    assert_eq!(g.undecided, 0, "a decided fill is not also undecided");
    assert_eq!(there.verdict, "fail");
}

/// **The undecided verdict refuses nothing.** A red gate keeps the `.nbt` off
/// disk; this one must not, or the third answer is a fail with a softer name and
/// an author routes around it inside a week.
#[test]
fn an_undecided_report_still_freezes_a_prefab() {
    let p = program(Some(largest_z()), Paint::block(state(BARS)));
    let report = judge(&p, DECLARED);
    assert_eq!(report.verdict, "undecided");

    let frozen = export_zone(
        &p,
        Box3::at_origin(DECLARED),
        &ExpandOptions::seeded(1),
        "undecided-demo",
    );
    assert!(
        frozen.is_ok(),
        "an undecided gate must not refuse an artifact: {:?}",
        frozen.err()
    );
}

/// **No reorientation request, no question.** A scope no rule reorients stands
/// in the identity frame at every region there will ever be, so its world-frame
/// literal is unconditionally what the author wrote. This is the ordinary
/// building, and it is most of every corpus.
#[test]
fn a_fill_under_no_reorientation_is_decided_and_says_nothing() {
    let report = judge(&program(None, Paint::block(state(BARS))), DECLARED);
    let g = oriented(&report);
    assert!(g.passed(), "{}", g.detail);
    assert_eq!(g.undecided, 0, "{}", g.detail);
    assert_eq!(report.verdict, "pass");
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.contains("could not decide")),
        "{:?}",
        report.findings
    );
    // ...and the same at the transposed region: nothing about this program is a
    // fact about the box.
    let other = judge(&program(None, Paint::block(state(BARS))), TRANSPOSED);
    assert!(oriented(&other).passed());
}

/// **A request that names its axes outright is region-independent**, so it
/// leaves the reachable frame set exactly one frame wide and decides everything
/// it touches. Written as a request that resolves to the identity, because that
/// is the one an author reaches for to *undo* an outer turn — and the one a
/// check keyed on "is there a reorientation on the path" would have flagged.
#[test]
fn a_reorientation_that_reads_no_proportion_decides_at_every_region() {
    let fixed = Reorient::KEEP
        .x(AxisSpec::WorldX)
        .y(AxisSpec::WorldY)
        .z(AxisSpec::WorldZ);
    for region in [DECLARED, TRANSPOSED] {
        let report = judge(&program(Some(fixed), Paint::block(state(BARS))), region);
        let g = oriented(&report);
        assert!(g.passed(), "{region:?}: {}", g.detail);
        assert_eq!(g.undecided, 0, "{region:?}: {}", g.detail);
    }
}

/// **A request may itself pin what the state names.** `y: world_y` keeps every
/// reachable frame's vertical on the world's, so a pillar's `axis=y` cannot be
/// disturbed by any of them — even though some frame of the cube would lay it
/// flat. Judging against all 48 frames instead of the reachable ones put six of
/// the live campaign's eight zones into a state their authors could do nothing
/// about, which is why the frame set is computed rather than assumed.
#[test]
fn a_pinned_vertical_decides_a_vertical_pillar() {
    let report = judge(
        &program(Some(largest_z()), Paint::block(state(PILLAR))),
        DECLARED,
    );
    let g = oriented(&report);
    assert!(g.passed(), "{}", g.detail);
    assert_eq!(g.undecided, 0, "{}", g.detail);
    assert_eq!(report.verdict, "pass");

    // The pin is doing the work, and the proof is that removing it re-opens the
    // question: with the vertical free to move, the same pillar is undecided.
    let free = Reorient::KEEP.z(AxisSpec::Largest).y(AxisSpec::Smallest);
    let loosened = judge(&program(Some(free), Paint::block(state(PILLAR))), DECLARED);
    let g = oriented(&loosened);
    assert_eq!(g.state, GateState::Undecided, "{}", g.detail);
    assert!(g.detail.contains("axis=y"), "{}", g.detail);
}

/// **A frame-inert state is decided by anything.** `minecraft:stone` names no
/// direction, so no frame in any set can land it wrong and the reorientation
/// above it is beside the point.
#[test]
fn a_state_that_names_no_direction_is_never_undecided() {
    let report = judge(
        &program(Some(largest_z()), Paint::block(state("minecraft:stone"))),
        DECLARED,
    );
    let g = oriented(&report);
    assert!(g.passed(), "{}", g.detail);
    assert_eq!(g.undecided, 0, "{}", g.detail);
}

/// **The repair the message names actually works.** The same bars written in
/// the scope's own axis frame are resolved at fill time, so there is no literal
/// left for any frame to land wrong — decided at both regions, and the local
/// frame's own binding count says where the population went.
#[test]
fn wrapping_the_state_in_the_scopes_own_frame_decides_it_at_every_region() {
    let p = program(Some(largest_z()), Paint::local_block(state(BARS)));
    for region in [DECLARED, TRANSPOSED] {
        let report = judge(&p, region);
        let g = oriented(&report);
        assert!(g.passed(), "{region:?}: {}", g.detail);
        assert_eq!(g.undecided, 0, "{region:?}: {}", g.detail);
        assert_eq!(report.verdict, "pass");
        assert!(
            report.measurements.local_frame_fills > 0,
            "{region:?}: the local frame's binding count must show where the population went"
        );
    }
}

/// The third answer is a function of the program and the region, like every
/// other verdict here: same input, byte-identical report (ADR-0006).
#[test]
fn the_undecided_verdict_is_deterministic() {
    let p = program(Some(largest_z()), Paint::block(state(BARS)));
    let a = judge(&p, DECLARED);
    let b = judge(&p, DECLARED);
    assert_eq!(a.to_json(), b.to_json());
    assert!(
        a.to_json().contains("\"state\": \"undecided\""),
        "{}",
        a.to_json()
    );
}
