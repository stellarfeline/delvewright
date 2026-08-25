//! **One rule, two arms, and the hole nothing reads.**
//!
//! A transept has two arms off a crossing, and they are each other's mirror
//! image, so one rule serves both. A `split` lays its pieces from the low end of
//! the axis, so a body of `[absolute 2 → end wall, relative → interior]` puts the
//! end wall at the arm's local `X`-min — the *outer* face of the near arm, and
//! the *crossing-facing* face of the far one. The far arm's outer face is then
//! nothing at all: a hole in the flank of the building, open to the sky.
//!
//! The measured part, and the reason this file exists rather than a note: with
//! the hole present, `blocks-exist` and `non-empty` are green over it. Nothing
//! in the toolchain could tell an author the copy they did not write is
//! missing, because the language had no way to say "this body, mirrored" and no
//! gate asked whether the two halves agreed. `traversable` reads this
//! building's misplaced wall — see the red test — but by its geometry and not
//! as the general finding: it is the shape of the arm being sealed that it
//! sees, never the fact that two halves disagree.
//!
//! Both halves of that are demonstrated here on one program: the frame's
//! `mirror` puts the far arm's end wall where it belongs, and the `symmetric`
//! gate reds on the version that does not use it.

use delvewright_grammar::block::BlockState;
use delvewright_grammar::gates;
use delvewright_grammar::geom::{Axis, Mirror, Orientation};
use delvewright_grammar::ir::{
    Alternative, Cond, Mark, MarkAt, Node, Program, Reorient, Rounding, Side, Size, Split,
    WeightedBlock,
};
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

/// A transept: two arms off a crossing, both arms expanded from ONE rule.
///
/// `second_arm` is the only difference between the two programs below — the
/// same `call("arm")`, bare or reflected.
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
        // The end wall is the FIRST piece, so it lands at the arm's local X-min.
        // Which world face that is, is the whole question.
        .rule(
            "arm",
            split(
                Axis::X,
                vec![Size::abs(2), Size::rel(1)],
                vec![
                    Node::Mark {
                        mark: Mark::new(
                            "portal",
                            MarkAt::FaceCenter {
                                axis: Axis::X,
                                side: Side::Min,
                            },
                        )
                        .indexed(),
                        body: Box::new(Node::fill("mass")),
                    },
                    Node::call("chamber"),
                ],
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

/// The far arm as one rule reflected — the whole fix, in one node.
fn reflected_arm() -> Node {
    Node::Reorient {
        orient: Reorient::KEEP.flip(Axis::X),
        body: Box::new(Node::call("arm")),
    }
}

fn run(program: &Program) -> Expansion {
    expand(program, Box3::at_origin(REGION), &ExpandOptions::seeded(1)).expect("expands")
}

fn judged(out: &Expansion, symmetric: Option<Axis>) -> gates::Report {
    gates::judge(
        out,
        gates::Options {
            traversable: true,
            allow_falls: false,
            symmetric,
            reachable_floor: false,
        },
    )
}

fn gate<'a>(report: &'a gates::Report, id: &str) -> &'a gates::Gate {
    report.gates.iter().find(|g| g.id == id).expect("gate ran")
}

/// Air cells in the region's `x == plane` face, excluding the floor, the ceiling
/// and the two `Z` walls — i.e. cells of the building's own flank.
fn flank_holes(out: &Expansion, plane: i32) -> usize {
    let mut n = 0;
    for y in 1..REGION[1] as i32 - 1 {
        for z in 1..REGION[2] as i32 - 1 {
            if out.model.get([plane, y, z]).is_some_and(|b| b.is_air()) {
                n += 1;
            }
        }
    }
    n
}

/// **The red.** One rule at both sites, unreflected: the far arm's end wall
/// faces the crossing and its outer face is 36 cells of open air.
///
/// The gates that cannot see it are `blocks-exist` and `non-empty`, and the
/// reason is the general one: nothing in them asks whether the two halves of a
/// shape that claims a mirror plane agree. That is `symmetric`'s question and
/// this file's subject.
///
/// **`traversable` used to be on that list and it is not any more, and the
/// change is worth stating because it is the same defect one layer out.** The
/// walk gate read the region's world `Z`-max and `Z`-min planes and nothing
/// else, so over this building it examined the crossing and called it
/// connected — a true statement about the two faces it looked at, on a
/// building whose east flank it never asked about. Asked which faces the piece
/// actually opens on, it finds three, and it finds the east one severed:
/// putting the far arm's end wall against the crossing does not only open the
/// flank, it seals the arm off from the rest of the building. The gate now
/// reads THIS INSTANCE, by geometry. It still does not read the general
/// finding — a flank hole that severed nothing would leave every gate here
/// green but `symmetric`, which is why that gate exists.
#[test]
fn red_one_rule_at_both_sites_opens_the_far_flank_and_the_shape_gates_stay_green() {
    let out = run(&transept(Node::call("arm")));

    assert_eq!(
        flank_holes(&out, 0),
        0,
        "the near arm's end wall IS its outer face"
    );
    assert_eq!(
        flank_holes(&out, REGION[0] as i32 - 1),
        36,
        "and the far arm's outer face is a hole"
    );

    let report = judged(&out, None);
    for id in ["blocks-exist", "non-empty"] {
        let g = gate(&report, id);
        assert!(
            g.passed() && g.bound > 0,
            "{id} is about blocks, not about halves: {}",
            g.detail
        );
    }

    let walk = gate(&report, "traversable");
    assert!(
        !walk.passed(),
        "the misplaced end wall seals the far arm, and a gate that asks the piece \
         which faces it opens on sees that: {}",
        walk.detail
    );
    assert_eq!(walk.bound, 3, "north, south and the opened east flank");
    assert!(
        walk.detail.contains("east side"),
        "the flank it could not see before is the one it names: {}",
        walk.detail
    );
}

/// **The green.** The same one rule, the far site reflected: the end wall lands
/// on the outer face of both arms, and the flank is closed.
#[test]
fn green_the_reflected_arm_puts_its_end_wall_on_the_outside() {
    let out = run(&transept(reflected_arm()));

    assert_eq!(flank_holes(&out, 0), 0);
    assert_eq!(
        flank_holes(&out, REGION[0] as i32 - 1),
        0,
        "the reflected arm's first split piece is its world-highest, so the end \
         wall is the outer face here too"
    );
    assert!(judged(&out, Some(Axis::X)).is_pass());
}

/// **The diagnostic's own red → green.** The `symmetric` gate is the general
/// form of the finding: not "this program forgot a mirror" but "the two halves
/// of a shape that claims a mirror plane disagree". It reds on the unreflected
/// program and passes on the reflected one, and it says how many cell pairs it
/// examined either way.
#[test]
fn the_symmetry_gate_reads_the_hole_the_other_gates_cannot() {
    let broken = judged(&run(&transept(Node::call("arm"))), Some(Axis::X));
    let g = gate(&broken, "symmetric");
    assert!(!g.passed(), "{}", g.detail);
    assert_eq!(g.bound, 660, "10 pairs per column, 6 x 11 columns");
    assert!(g.detail.contains("differ"), "{}", g.detail);
    assert!(!broken.is_pass());

    let whole = judged(&run(&transept(reflected_arm())), Some(Axis::X));
    let g = gate(&whole, "symmetric");
    assert!(g.passed(), "{}", g.detail);
    assert_eq!(g.bound, 660);
    assert!(whole.is_pass());
}

/// A gate that examined nothing is a finding, not a pass — the vacuity mode
/// CLAUDE.md names, applied to this gate. A one-cell axis has no pairs.
#[test]
fn the_symmetry_gate_over_a_one_cell_axis_binds_to_nothing_and_says_so() {
    let program = Program::new("slab", "all")
        .role("mass", BlockState::simple("stone_bricks"))
        .rule("all", Node::fill("mass"));
    let out = expand(
        &program,
        Box3::at_origin([1, 3, 3]),
        &ExpandOptions::seeded(0),
    )
    .expect("expands");
    let report = gates::judge(
        &out,
        gates::Options {
            traversable: false,
            allow_falls: false,
            symmetric: Some(Axis::X),
            reachable_floor: false,
        },
    );
    let g = gate(&report, "symmetric");
    assert_eq!(g.bound, 0);
    assert!(!g.passed(), "a zero binding is not a pass");
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.contains("`symmetric`") && f.contains("ZERO")),
        "{:?}",
        report.findings
    );
}

/// **The gate compares presence, not block state.** A stair placed correctly on
/// both sides of a mirror plane is a *different* state on each side, because
/// nothing reflects a `facing=` property. Comparing states would red every
/// symmetric building that contains one.
#[test]
fn the_symmetry_gate_compares_presence_because_a_facing_does_not_reflect() {
    let program = Program::new("pair", "run")
        .role(
            "west",
            BlockState::with("stone_brick_stairs", [("facing", "west")]),
        )
        .role(
            "east",
            BlockState::with("stone_brick_stairs", [("facing", "east")]),
        )
        .rule(
            "run",
            split(
                Axis::X,
                vec![Size::abs(1), Size::rel(1), Size::abs(1)],
                vec![Node::fill("west"), Node::Void, Node::fill("east")],
            ),
        );
    let out = expand(
        &program,
        Box3::at_origin([5, 2, 2]),
        &ExpandOptions::seeded(0),
    )
    .expect("expands");
    let report = gates::judge(
        &out,
        gates::Options {
            traversable: false,
            allow_falls: false,
            symmetric: Some(Axis::X),
            reachable_floor: false,
        },
    );
    assert!(
        gate(&report, "symmetric").passed(),
        "two facings, one silhouette"
    );
}

/// A mark under a reflection lands on the mirror image of the cell it names, and
/// derives the mirror image of the facing — so the anchor a rule declares at the
/// mouth of one arm is at the mouth of the other, looking out of it.
#[test]
fn a_mark_under_a_reflection_lands_on_the_mirror_image_cell() {
    let out = run(&transept(reflected_arm()));
    let near = &out.anchors["anchor/portal-1"];
    let far = &out.anchors["anchor/portal-2"];

    assert_eq!(near.pos[0], 0, "the near arm's end wall, outer face");
    assert_eq!(
        far.pos[0],
        REGION[0] as i32 - 1,
        "and the far arm's, which is the mirror image cell"
    );
    assert_eq!(near.pos[1..], far.pos[1..], "same height, same depth");
    // Both scopes call world Z their local Z; the reflection is on X, so neither
    // facing moves. The reflection that DOES move a facing is on the axis local
    // Z names — asserted below.
    assert_eq!(near.facing.as_str(), "north");
    assert_eq!(far.facing.as_str(), "north");
}

/// **A derived facing follows the frame's direction, not just its mapping.** The
/// rule library's frame says travel runs toward local `Z`-min, so a mark looks
/// that way; reflect local `Z` and it looks the other way. All four cardinals are
/// reachable, which no permutation alone could give.
#[test]
fn a_derived_facing_follows_the_reflection() {
    let program = Program::new("look", "here").rule(
        "here",
        Node::Mark {
            mark: Mark::new("eye", MarkAt::CornerMin),
            body: Box::new(Node::Skip),
        },
    );
    let facing = |orient: Orientation| {
        expand(
            &program,
            Box3::at_origin([3, 3, 3]),
            &ExpandOptions {
                seed: 0,
                limits: Default::default(),
                orientation: orient,
            },
        )
        .expect("expands")
        .anchors["anchor/eye"]
            .facing
            .as_str()
            .to_string()
    };
    let along_z = Orientation::IDENTITY;
    let along_x = Orientation::from_axes([Axis::Z, Axis::Y, Axis::X]);
    assert_eq!(facing(along_z), "north");
    assert_eq!(facing(along_z.mirrored(Mirror::of(Axis::Z))), "south");
    assert_eq!(facing(along_x), "west");
    assert_eq!(facing(along_x.mirrored(Mirror::of(Axis::Z))), "east");
}

/// **Reflecting twice is the identity**, so a rule may be reflected at any depth
/// without knowing whether it already is: the model does not move.
#[test]
fn a_doubly_reflected_body_is_the_body() {
    let plain = run(&transept(Node::call("arm")));
    let twice = run(&transept(Node::Reorient {
        orient: Reorient::KEEP.flip(Axis::X),
        body: Box::new(Node::Reorient {
            orient: Reorient::KEEP.flip(Axis::X),
            body: Box::new(Node::call("arm")),
        }),
    }));
    assert_eq!(
        plain.model.canonical_bytes(),
        twice.model.canonical_bytes(),
        "two reflections cancel"
    );
}

/// **A frame guard tells the two sides of a mirror pair apart**, which is what
/// makes a directional block state usable under a reflection: the guard matches
/// the frame entire, so an unqualified one holds only in the unreflected scope.
#[test]
fn a_frame_guard_picks_the_state_the_reflected_side_wants() {
    let program = Program::new("treads", "run")
        .role(
            "west",
            BlockState::with("stone_brick_stairs", [("facing", "west")]),
        )
        .role(
            "east",
            BlockState::with("stone_brick_stairs", [("facing", "east")]),
        )
        .rule(
            "run",
            split(
                Axis::X,
                vec![Size::rel(1), Size::rel(1)],
                vec![
                    Node::call("tread"),
                    Node::Reorient {
                        orient: Reorient::KEEP.flip(Axis::X),
                        body: Box::new(Node::call("tread")),
                    },
                ],
            ),
        )
        .rule_alts(
            "tread",
            vec![
                Alternative::new(Node::fill("west")).when(Cond::orientation(
                    Axis::X,
                    Axis::Y,
                    Axis::Z,
                )),
                Alternative::new(Node::fill("east")).when(Cond::Otherwise),
            ],
        );
    let out = expand(
        &program,
        Box3::at_origin([4, 1, 1]),
        &ExpandOptions::seeded(0),
    )
    .expect("expands");
    let facing = |x: i32| out.model.get([x, 0, 0]).expect("in region").properties["facing"].clone();
    assert_eq!(facing(0), "west", "the unreflected half");
    assert_eq!(
        facing(3),
        "east",
        "the reflected half falls to `otherwise`, because an unqualified frame \
         guard asks for an unreflected scope"
    );
}

/// **Determinism (ADR-0006).** Same program, same region, same seed, byte for
/// byte — and a reflection is data, so it changes the model and not the promise.
#[test]
fn a_reflected_program_is_byte_identical_across_runs() {
    let a = run(&transept(reflected_arm()));
    let b = run(&transept(reflected_arm()));
    assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    assert_eq!(a.anchors, b.anchors);
    assert_ne!(
        a.model.canonical_bytes(),
        run(&transept(Node::call("arm"))).model.canonical_bytes(),
        "and the reflection is not a no-op"
    );
}

/// **Determinism where a reflection could plausibly break it.** A reflected
/// split visits its pieces in pattern order, which is now world-high to
/// world-low, while a mix draws per cell in the model's own world order — two
/// orders that a reflection puts out of step. Both are fixed, so the stream is
/// the seed's and only the seed's.
#[test]
fn a_reflected_probabilistic_program_follows_its_seed_and_only_its_seed() {
    let program = Program::new("scatter", "run")
        .role_mix(
            "rubble",
            vec![
                WeightedBlock {
                    weight: 3,
                    block: BlockState::simple("minecraft:stone_bricks"),
                },
                WeightedBlock {
                    weight: 1,
                    block: BlockState::simple("minecraft:mossy_stone_bricks"),
                },
                WeightedBlock {
                    weight: 1,
                    block: BlockState::simple("minecraft:air"),
                },
            ],
        )
        .rule(
            "run",
            split(
                Axis::X,
                vec![Size::rel(1), Size::rel(1)],
                vec![
                    Node::call("band"),
                    Node::Reorient {
                        orient: Reorient::KEEP.flip(Axis::X),
                        body: Box::new(Node::call("band")),
                    },
                ],
            ),
        )
        .rule(
            "band",
            split(
                Axis::X,
                vec![Size::abs(1), Size::rel(1)],
                vec![Node::fill("rubble"), Node::fill("rubble")],
            ),
        );
    let at = |seed: u64| {
        expand(
            &program,
            Box3::at_origin([9, 4, 4]),
            &ExpandOptions::seeded(seed),
        )
        .expect("expands")
        .model
        .canonical_bytes()
    };
    assert_eq!(at(7), at(7), "same seed, same bytes");
    assert_ne!(at(7), at(8), "and the seed is a real control");
}
