//! **A reflected frame under a world-space walk** — the pair neither branch had.
//!
//! `#413` makes a frame carry a direction, so one rule stands at both sites of a
//! mirror plane. `#414` measures how much of a piece's floor a body reaches, and
//! adds `--reachable-floor` beside `--symmetric`. Neither branch could test the
//! other: the reflection did not exist when the measurement was written, and the
//! measurement did not exist when the reflection was.
//!
//! The interaction is not hypothetical. The reachability measurement is a walk in
//! **world** space — `nav::ground_entry` finds the entrance by looking at world
//! side faces at world grade, and `nav::components` walks world neighbours. The
//! reflection is a **frame** transform, entirely upstream of it. If any of that
//! walk had a direction preference, a building and its mirror image would report
//! different numbers while being the same building seen from the other side.
//!
//! Four claims, and what each would look like if it were false:
//!
//! 1. Both opt-in gates run on one expansion, each binding to its own set. If
//!    `judge` had been written to take one optional gate, the second silently
//!    would not appear.
//! 2. A doubly-reflected program is byte-identical to the plain one (reflection
//!    is XOR), so the whole report must be identical — not just the geometry. A
//!    measurement keyed to expansion ORDER rather than to world geometry would
//!    differ here while the `.nbt` matched.
//! 3. A building and its reflection give the same reachability numbers. A walk
//!    with a direction preference — an entrance search that only looked at one
//!    face, a component walk whose tie-break was positional — would not.
//! 4. The newest gate in the subsystem is blind to the defect the other exists
//!    for. Across a transept with a 7x4x9 hole in one flank and the same
//!    transept with its far arm reflected, `symmetric` flips red to green and
//!    `reachable-floor` does not move — same verdict, same binding count, green
//!    over both. Neither gate stands in for the other.

use delvewright_grammar::block::BlockState;
use delvewright_grammar::gates::{self, Options};
use delvewright_grammar::geom::Axis;
use delvewright_grammar::ir::{Node, Program, Reorient, Rounding, Size, Split};
use delvewright_grammar::{Box3, ExpandOptions, Expansion, expand};

const REGION: [u32; 3] = [21, 6, 11];

fn split(axis: Axis, sizes: Vec<Size>, children: Vec<Node>) -> Node {
    Node::Split(Split {
        axis,
        sizes,
        rounding: Rounding::Start,
        repeat: false,
        orient: Reorient::KEEP,
        children,
    })
}

/// A transept: two arms off a crossing, both arms expanded from ONE rule, with a
/// hollow chamber inside each arm. `second_arm` is the only difference between
/// the programs compared below.
fn transept(second_arm: Node) -> Program {
    Program::new("transept", "transept")
        .role("mass", BlockState::simple("stone_bricks"))
        .rule(
            "transept",
            split(
                Axis::X,
                vec![Size::abs(7), Size::rel(1), Size::abs(7)],
                vec![Node::call("arm"), Node::call("crossing"), second_arm],
            ),
        )
        .rule(
            "arm",
            split(
                Axis::X,
                vec![Size::abs(2), Size::rel(1)],
                vec![Node::fill("mass"), Node::call("chamber")],
            ),
        )
        .rule(
            "chamber",
            split(
                Axis::Y,
                vec![Size::abs(1), Size::rel(1), Size::abs(1)],
                vec![
                    Node::fill("mass"),
                    Node::call("chamber_walls"),
                    Node::fill("mass"),
                ],
            ),
        )
        .rule(
            "chamber_walls",
            split(
                Axis::Z,
                vec![Size::abs(1), Size::rel(1), Size::abs(1)],
                vec![Node::fill("mass"), Node::Void, Node::fill("mass")],
            ),
        )
        .rule(
            "crossing",
            split(
                Axis::Y,
                vec![Size::abs(1), Size::rel(1), Size::abs(1)],
                vec![Node::fill("mass"), Node::Void, Node::fill("mass")],
            ),
        )
}

fn reflect(times: usize, body: Node) -> Node {
    (0..times).fold(body, |acc, _| Node::Reorient {
        orient: Reorient::KEEP.flip(Axis::X),
        body: Box::new(acc),
    })
}

fn run(program: &Program) -> Expansion {
    expand(program, Box3::at_origin(REGION), &ExpandOptions::seeded(1)).expect("expands")
}

fn both_gates(symmetric: Option<Axis>) -> Options {
    Options {
        traversable: false,
        allow_falls: false,
        symmetric,
        reachable_floor: true,
    }
}

/// Claim 1: the two opt-in gates coexist, each with its own binding count, and
/// the report carries both.
#[test]
fn the_symmetry_gate_and_the_reachability_gate_run_on_one_expansion() {
    let out = run(&transept(reflect(1, Node::call("arm"))));
    let report = gates::judge(&out, both_gates(Some(Axis::X)));

    let ids: Vec<&str> = report.gates.iter().map(|g| g.id).collect();
    assert!(ids.contains(&"symmetric"), "{ids:?}");
    assert!(ids.contains(&"reachable-floor"), "{ids:?}");
    assert_eq!(
        report.gates.len(),
        4,
        "two always-on gates plus the two opt-in ones: {ids:?}"
    );
    for gate in &report.gates {
        assert!(
            gate.bound > 0,
            "gate `{}` examined zero objects: {}",
            gate.id,
            gate.detail
        );
    }
    println!(
        "cross-gate      bound {:3}  gate(s), {}",
        report.gates.len(),
        report
            .gates
            .iter()
            .map(|g| format!("{}={}", g.id, g.bound))
            .collect::<Vec<_>>()
            .join(" ")
    );
}

/// Claim 2: reflection composes by exclusive-or, so a doubly-reflected body is
/// the plain one — and the whole report, measurement included, must agree, not
/// only the blocks.
#[test]
fn a_doubly_reflected_body_reports_identically_to_the_plain_one() {
    let plain = run(&transept(Node::call("arm")));
    let twice = run(&transept(reflect(2, Node::call("arm"))));

    let cells = Box3::at_origin(REGION).positions().count();
    let differing = Box3::at_origin(REGION)
        .positions()
        .filter(|&p| plain.model.get(p) != twice.model.get(p))
        .count();
    assert_eq!(differing, 0, "{differing} of {cells} cells differ");

    let opts = both_gates(None);
    assert_eq!(
        gates::judge(&plain, opts).to_json(),
        gates::judge(&twice, opts).to_json(),
        "the reports must agree cell for cell and number for number"
    );
    println!("double-mirror   bound {cells:3}  cell(s) compared, 0 differ; reports identical");
}

/// Claim 3: the reachability measurement is a fact about world geometry, so a
/// building and its mirror image give the same numbers.
///
/// The two programs here are genuinely different worlds — the asymmetric
/// transept and its reflection about `X` — which is the point: if the answers
/// were equal only because the shape was symmetric, the test would prove
/// nothing.
#[test]
fn a_building_and_its_reflection_measure_the_same() {
    let plain = run(&transept(Node::call("arm")));
    let mut flipped_program =
        transept(Node::call("arm")).rule("flipped", reflect(1, Node::call("transept")));
    flipped_program.start = "flipped".to_string();
    let flipped = run(&flipped_program);
    // The reflected whole is a different world from the plain one …
    let differing = Box3::at_origin(REGION)
        .positions()
        .filter(|&p| plain.model.get(p) != flipped.model.get(p))
        .count();
    assert!(
        differing > 0,
        "the reflection must actually move blocks, or this proves nothing"
    );

    // … and every reachability number about it is the same.
    let a = gates::judge(&plain, both_gates(None));
    let b = gates::judge(&flipped, both_gates(None));
    let (ra, rb) = (&a.measurements.reachability, &b.measurements.reachability);
    assert_eq!(ra.standable, rb.standable, "standable");
    assert_eq!(ra.entry_cells, rb.entry_cells, "entry_cells");
    assert_eq!(ra.reachable, rb.reachable, "reachable");
    assert_eq!(ra.sheltered, rb.sheltered, "sheltered");
    assert_eq!(
        ra.unreachable_sheltered, rb.unreachable_sheltered,
        "unreachable_sheltered"
    );
    assert_eq!(ra.pockets, rb.pockets, "pockets");
    assert!(
        ra.standable > 0 && ra.entry_cells > 0,
        "a measurement over nothing would match trivially: standable {} entry {}",
        ra.standable,
        ra.entry_cells
    );
    println!(
        "mirror-invariant bound {:3}  standable cell(s), {} entry, {} reachable, {} moved by the \
         reflection",
        ra.standable, ra.entry_cells, ra.reachable, differing
    );
}

/// Claim 4: `#414`'s gate is **blind to the defect `#413` exists for**, so the
/// two verdicts are independent and neither stands in for the other.
///
/// The pair is the motivating story itself: one transept whose far arm is a bare
/// `call` — the end wall on the wrong face, a 7x4x9 opening in the flank — and
/// one whose far arm is that same rule reflected. Across that pair `symmetric`
/// flips red to green while `reachable-floor` does not move at all: same verdict,
/// same binding count, on a building with a hole in its side and on one without.
/// That is what "green over a hole" looked like, measured against the newest gate
/// in the subsystem rather than against the ones the original finding named.
#[test]
fn the_floor_gate_cannot_see_the_hole_the_symmetry_gate_exists_for() {
    let holed = gates::judge(
        &run(&transept(Node::call("arm"))),
        both_gates(Some(Axis::X)),
    );
    let whole = gates::judge(
        &run(&transept(reflect(1, Node::call("arm")))),
        both_gates(Some(Axis::X)),
    );
    let gate = |r: &gates::Report, id: &str| {
        let g = r.gates.iter().find(|g| g.id == id).expect("gate ran");
        (g.pass, g.bound)
    };

    assert!(!gate(&holed, "symmetric").0, "the hole must red it");
    assert!(gate(&whole, "symmetric").0, "the reflection must green it");
    assert_eq!(
        gate(&holed, "reachable-floor"),
        gate(&whole, "reachable-floor"),
        "the floor gate must not move across the pair — if it did, this test would be \
         proving the two gates overlap rather than that they do not"
    );
    assert!(
        gate(&holed, "reachable-floor").0,
        "and it is GREEN over the holed building, which is the whole point"
    );
    assert!(gate(&holed, "reachable-floor").1 > 0, "on a real binding");
    println!(
        "gate-independence bound {:3}  reachable-floor cell(s), identical over both \
         buildings; symmetric {} -> {}",
        gate(&holed, "reachable-floor").1,
        gate(&holed, "symmetric").0,
        gate(&whole, "symmetric").0
    );
}
