//! **The idiom index is held to what it teaches.**
//!
//! `docs/reference/grammar.md` §2c documents ten techniques of the IR, each
//! with a minimal program at a stated region and seed. A documented example that
//! stopped being true is worse than no example: an author starts from the corpus
//! (`prefab-procedure.md` §3), so the corpus is what they learn from.
//!
//! Every test here therefore does two things. It expands the documented program
//! at its **documented region and seed** — the same numbers the reference
//! prints, so a doc edit and a rule edit cannot drift apart — and it asserts the
//! claim that technique makes, with the count of what it examined. Several also
//! carry the red: the same program with the technique removed, measured going
//! wrong in exactly the way the entry warns about.
//!
//! The four IR facts at the end are not idioms. They are the things
//! `grammar.md` §2 now states that an author could previously only learn by
//! reading `crates/grammar/src/*`, and a stated fact with no test is the drift
//! this file exists to stop.

use std::collections::BTreeSet;

use delvewright_grammar::block::BlockState;
use delvewright_grammar::gates;
use delvewright_grammar::geom::Axis;
use delvewright_grammar::ir::{
    Alternative, AxisSpec, CmpOp, Cond, DimRef, Expr, Material, Node, Paint, Program, Reorient,
    Rounding, Size, Split, WeightedBlock,
};
use delvewright_grammar::library::{self, idioms};
use delvewright_grammar::{Box3, ExpandError, ExpandOptions, Expansion, expand};

// ---------------------------------------------------------------------------
// The documented table
// ---------------------------------------------------------------------------

/// One documented example: the id `delve-grammar list` prints, the program, the
/// region and seed the reference states, and whether the entry claims the piece
/// is a route.
struct Case {
    id: &'static str,
    program: fn() -> Program,
    region: [u32; 3],
    seed: u64,
    traversable: bool,
    /// The world axis this entry claims a mirror plane on, when it claims one.
    symmetric: Option<Axis>,
}

const CASES: &[Case] = &[
    Case {
        id: "idiom-repetition",
        program: idioms::repetition,
        region: [3, 5, 17],
        seed: 1,
        traversable: false,
        symmetric: None,
    },
    Case {
        id: "idiom-priority",
        program: idioms::priority,
        region: [13, 6, 2],
        seed: 1,
        traversable: false,
        symmetric: None,
    },
    Case {
        id: "idiom-shape",
        program: idioms::shape,
        region: [15, 9, 3],
        seed: 1,
        traversable: false,
        symmetric: None,
    },
    Case {
        id: "idiom-erosion",
        program: idioms::erosion,
        region: [9, 5, 3],
        seed: 1,
        traversable: false,
        symmetric: None,
    },
    Case {
        id: "idiom-erosion-graded",
        program: idioms::graded_erosion,
        region: [9, 13, 3],
        seed: 1,
        traversable: false,
        symmetric: None,
    },
    Case {
        id: "idiom-surface-detail",
        program: idioms::surface_detail,
        region: [9, 12, 9],
        seed: 1,
        traversable: false,
        symmetric: None,
    },
    Case {
        id: "idiom-mirror",
        program: idioms::mirror,
        region: [15, 11, 2],
        seed: 1,
        traversable: false,
        // The rose window IS the mirror plane, so the entry makes the claim and
        // the gate reads it — the technique is proved by the same run that
        // documents it, not only by a test that builds its own fixture.
        symmetric: Some(Axis::Y),
    },
    Case {
        id: "idiom-skip",
        program: idioms::skip,
        region: [7, 5, 5],
        seed: 1,
        traversable: true,
        symmetric: None,
    },
    Case {
        id: "idiom-light",
        program: idioms::light,
        region: [5, 6, 13],
        seed: 1,
        traversable: true,
        symmetric: None,
    },
    Case {
        id: "idiom-arguments",
        program: idioms::arguments,
        region: [15, 7, 15],
        seed: 1,
        traversable: false,
        symmetric: None,
    },
    Case {
        id: "idiom-composition-arcade",
        program: idioms::composition_arcade,
        region: [3, 14, 20],
        seed: 1,
        traversable: false,
        symmetric: None,
    },
];

fn expand_case(case: &Case) -> Expansion {
    expand(
        &(case.program)(),
        Box3::at_origin(case.region),
        &ExpandOptions::seeded(case.seed),
    )
    .unwrap_or_else(|e| panic!("{}: {e}", case.id))
}

/// Expand any program over any box at any seed.
fn run(program: &Program, size: [u32; 3], seed: u64) -> Expansion {
    expand(program, Box3::at_origin(size), &ExpandOptions::seeded(seed))
        .unwrap_or_else(|e| panic!("{}: {e}", program.name))
}

fn is_air(out: &Expansion, cell: [i32; 3]) -> bool {
    out.model.get(cell).map(|b| b.is_air()).unwrap_or(true)
}

fn block_at(out: &Expansion, cell: [i32; 3]) -> String {
    out.model
        .get(cell)
        .map(|b| b.name.clone())
        .unwrap_or_default()
}

/// Non-air cells of one `y` course, counted at every `z`.
fn course_width(out: &Expansion, size: [u32; 3], y: i32, z: i32) -> usize {
    (0..size[0] as i32)
        .filter(|&x| !is_air(out, [x, y, z]))
        .count()
}

/// Every cell of the region, in the model's own order.
fn cells(size: [u32; 3]) -> impl Iterator<Item = [i32; 3]> {
    (0..size[0] as i32).flat_map(move |x| {
        (0..size[1] as i32).flat_map(move |y| (0..size[2] as i32).map(move |z| [x, y, z]))
    })
}

// ---------------------------------------------------------------------------
// Promises every documented example owes
// ---------------------------------------------------------------------------

/// Every documented example expands **green at its documented region and seed**,
/// and every gate that ran examined something.
///
/// The binding count is asserted rather than only the verdict: a gate that
/// looked at zero objects is not a pass (CLAUDE.md), and the two always-on gates
/// plus the opt-in walk are exactly what `delve-grammar expand` prints, so this
/// is the same verdict an author reading the reference will get.
#[test]
fn every_documented_example_expands_green_at_its_documented_region() {
    let mut judged = 0usize;
    for case in CASES {
        let out = expand_case(case);
        let report = gates::judge(
            &out,
            gates::Options {
                traversable: case.traversable,
                allow_falls: false,
                symmetric: case.symmetric,
                reachable_floor: false,
            },
        );
        assert!(
            report.is_pass(),
            "{} went red: {:#?}",
            case.id,
            report.gates
        );
        assert_eq!(
            report.gates.len(),
            2 + usize::from(case.traversable) + usize::from(case.symmetric.is_some()),
            "{}",
            case.id
        );
        for gate in &report.gates {
            assert!(
                gate.bound > 0,
                "{}: gate `{}` examined zero objects — {}",
                case.id,
                gate.id,
                gate.detail
            );
            judged += 1;
        }
    }
    assert_eq!(
        judged, 25,
        "11 examples, 2 always-on gates, 2 walk gates, 1 mirror-plane gate"
    );
}

/// The documented ids are the ids the tool lists, and **every idiom the library
/// registers is documented here**.
///
/// The second direction is the one that rots: an idiom added to `PROGRAMS`
/// without a row in `CASES` would be a teaching program nothing expands, which
/// is how a corpus entry stops being true in silence.
#[test]
fn the_registry_and_the_documented_table_agree_in_both_directions() {
    let documented: BTreeSet<&str> = CASES.iter().map(|c| c.id).collect();
    let registered: BTreeSet<&str> = library::PROGRAMS
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| id.starts_with("idiom-"))
        .collect();
    assert_eq!(documented, registered);
    assert_eq!(documented.len(), 11);
    for case in CASES {
        let listed = library::by_id(case.id).unwrap_or_else(|| panic!("{} not listed", case.id));
        assert_eq!(listed, (case.program)(), "{}", case.id);
    }
}

/// ADR-0006 over the teaching set: same program, same region, same seed, same
/// bytes — and the JSON an author copies out of `delve-grammar show` expands to
/// the same model as the Rust it came from.
#[test]
fn every_documented_example_is_deterministic_and_survives_json() {
    for case in CASES {
        let program = (case.program)();
        for seed in [0u64, case.seed, 7] {
            let a = run(&program, case.region, seed);
            let b = run(&program, case.region, seed);
            assert_eq!(
                a.model.canonical_bytes(),
                b.model.canonical_bytes(),
                "{} at seed {seed}",
                case.id
            );
            assert_eq!(a.anchors, b.anchors, "{} at seed {seed}", case.id);
        }
        let json = serde_json::to_string_pretty(&program).unwrap();
        let back: Program = serde_json::from_str(&json).unwrap();
        assert_eq!(back, program, "{} did not survive JSON", case.id);
        let from_json = run(&back, case.region, case.seed);
        let from_rust = run(&program, case.region, case.seed);
        assert_eq!(
            from_json.model.canonical_bytes(),
            from_rust.model.canonical_bytes(),
            "{}",
            case.id
        );
    }
}

// ---------------------------------------------------------------------------
// 1. Repetition
// ---------------------------------------------------------------------------

/// **The two forms of repetition agree, cell for cell.**
///
/// The `-X` lane tiles its pattern with a `repeat` split; the `+X` lane peels
/// one pier and one bay and calls itself on the remainder. At the documented
/// region they are the same rhythm, which is the entry's point: `repeat` is the
/// right form whenever no step needs to know how far along it is.
#[test]
fn repetition_the_tiling_and_the_recursion_lay_the_same_rhythm() {
    let case = &CASES[0];
    let size = case.region;
    let out = expand_case(case);

    let mut compared = 0usize;
    for y in 0..size[1] as i32 {
        for z in 0..size[2] as i32 {
            assert_eq!(
                is_air(&out, [0, y, z]),
                is_air(&out, [2, y, z]),
                "the lanes differ at y={y} z={z}"
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 85, "5 courses x 17 cells of length");

    // ...and the rhythm is the documented one: a pier every `pier + bay`.
    let piers: Vec<i32> = (0..size[2] as i32)
        .filter(|&z| !is_air(&out, [0, 0, z]))
        .collect();
    assert_eq!(piers, vec![0, 4, 8, 12, 16]);
    // The gap between the lanes is written as air by a `void`, not left over.
    assert!((0..size[2] as i32).all(|z| is_air(&out, [1, 0, z])));
}

/// **A recursion without a base case ends in `NoApplicableRule`.**
///
/// The red the entry warns about, run: strip the `otherwise` arm off
/// `recursed_row` and the expansion stops the first time the remainder is too
/// short for another pier and bay.
#[test]
fn repetition_without_its_otherwise_arm_is_a_refusal() {
    let mut program = idioms::repetition();
    strip_otherwise(&mut program, "recursed_row");
    let err = expand(
        &program,
        Box3::at_origin(CASES[0].region),
        &ExpandOptions::seeded(1),
    )
    .unwrap_err();
    assert!(
        matches!(&err, ExpandError::NoApplicableRule { symbol } if symbol == "recursed_row"),
        "{err}"
    );
}

// ---------------------------------------------------------------------------
// 2. Priority
// ---------------------------------------------------------------------------

/// **Three bays, three arms, one each — and the third is the `otherwise`.**
#[test]
fn priority_each_arm_fires_on_the_bay_its_guard_describes() {
    let case = &CASES[1];
    let out = expand_case(case);

    // Bay 0 (7 wide): jambs, a 5-wide opening, a lintel course over all of it.
    assert_eq!(
        course_width(&out, case.region, 4, 0),
        6,
        "two jambs, the slot's two jambs, and the solid pier"
    );
    assert!((1..=5).all(|x| is_air(&out, [x, 4, 0])), "the wide opening");
    assert!(
        (0..=6).all(|x| block_at(&out, [x, 5, 0]) == "minecraft:chiseled_stone_bricks"),
        "the lintel is the arm's own material"
    );

    // Bay 1 (4 wide): a 2-wide slot with a sill under it and a head over it.
    assert!((8..=9).all(|x| is_air(&out, [x, 3, 0])), "the slot");
    assert!((7..=10).all(|x| !is_air(&out, [x, 0, 0])), "its sill");
    assert!((7..=10).all(|x| !is_air(&out, [x, 5, 0])), "its head");

    // Bay 2 (2 wide): neither guard holds, so the `otherwise` fills it solid.
    let solid = (11..=12)
        .flat_map(|x| (0..6).map(move |y| [x, y, 0]))
        .filter(|&c| !is_air(&out, c))
        .count();
    assert_eq!(solid, 12, "the narrow bay is a solid pier");
}

/// **Two guards that can both hold are a probability, not a priority** — the
/// measured red behind the entry's first warning.
///
/// Widen the slot arm's guard to `X >= slot_min`, dropping the `X < arch_min`
/// half, and the wide bay is no longer decided: both arms hold, and the seed
/// picks between them. Twelve seeds produce more than one building.
#[test]
fn priority_overlapping_guards_are_a_weighted_draw() {
    let sound = idioms::priority();
    let mut overlapping = sound.clone();
    let alts = overlapping.rules.get_mut("bay").unwrap();
    alts[1].when = Cond::cmp(Expr::dim(DimRef::X), CmpOp::Ge, Expr::param("slot_min"));

    let region = CASES[1].region;
    let sound_shapes: BTreeSet<Vec<u8>> = (0..12)
        .map(|s| run(&sound, region, s).model.canonical_bytes())
        .collect();
    let drawn_shapes: BTreeSet<Vec<u8>> = (0..12)
        .map(|s| run(&overlapping, region, s).model.canonical_bytes())
        .collect();

    assert_eq!(
        sound_shapes.len(),
        1,
        "exclusive guards are a decision: the same box always builds the same bay"
    );
    assert!(
        drawn_shapes.len() > 1,
        "overlapping guards are a draw, so the same box must NOT always build the same bay"
    );
}

// ---------------------------------------------------------------------------
// 3. Shape
// ---------------------------------------------------------------------------

/// **The profile is arithmetic on the remaining dimension, and the arithmetic
/// shows.**
///
/// The step is `max(1, X / run)`, so the wide courses at the foot step in two
/// cells a side and the narrow ones at the head step in one. A fixed `1` would
/// give 15, 13, 11, … — the constant 45° wedge the trial reported as the only
/// available profile.
#[test]
fn shape_the_taper_step_follows_the_remaining_width() {
    let case = &CASES[2];
    let out = expand_case(case);
    let widths: Vec<usize> = (0..case.region[1] as i32)
        .map(|y| course_width(&out, case.region, y, 0))
        .collect();
    assert_eq!(widths, vec![15, 11, 9, 7, 5, 3, 1, 1, 1]);
    assert_ne!(
        widths[1], 13,
        "a constant one-cell step would give 13 here; the step is arithmetic"
    );
}

/// **A pitched roof and a pointed arch are the same program with the paint
/// inverted** — measured as an exact complement over every cell of the region.
#[test]
fn shape_inverting_the_palette_turns_the_roof_into_the_opening() {
    let case = &CASES[2];
    let roof = expand_case(case);

    let mut arch_program = idioms::shape();
    arch_program
        .set_role("mass", Paint::Block(BlockState::air()))
        .unwrap();
    arch_program
        .set_role("cut", Paint::Block(BlockState::simple("stone_bricks")))
        .unwrap();
    let arch = run(&arch_program, case.region, case.seed);

    let mut examined = 0usize;
    for cell in cells(case.region) {
        assert_ne!(
            is_air(&roof, cell),
            is_air(&arch, cell),
            "the two expansions are not complements at {cell:?}"
        );
        examined += 1;
    }
    assert_eq!(examined, 405, "15 x 9 x 3");
    assert_eq!(
        roof.model.filled_cells() + arch.model.filled_cells(),
        examined
    );
}

// ---------------------------------------------------------------------------
// 4. Erosion
// ---------------------------------------------------------------------------

/// **Air is a legal member of a weighted role, and it is the whole of decay.**
///
/// The red is the same program with the air member taken out: the wall goes
/// solid, and nothing else about it moves.
#[test]
fn erosion_air_in_a_mix_is_what_voids_the_cells() {
    let case = &CASES[3];
    let out = expand_case(case);
    let volume = 9 * 5 * 3;
    let voided = volume - out.model.filled_cells();
    assert!(
        (8..=30).contains(&voided),
        "{voided} of {volume} cells voided at a 2-in-16 air weight"
    );
    let solids: BTreeSet<String> = out
        .model
        .palette()
        .iter()
        .filter(|b| !b.is_air())
        .map(|b| b.name.clone())
        .collect();
    assert_eq!(solids.len(), 3, "three masonry members: {solids:?}");

    let mut sound = idioms::erosion();
    sound
        .set_role(
            "ruin",
            Paint::Mix(vec![
                WeightedBlock {
                    weight: 9,
                    block: BlockState::simple("stone_bricks"),
                },
                WeightedBlock {
                    weight: 3,
                    block: BlockState::simple("mossy_stone_bricks"),
                },
                WeightedBlock {
                    weight: 2,
                    block: BlockState::simple("cracked_stone_bricks"),
                },
            ]),
        )
        .unwrap();
    assert_eq!(
        run(&sound, case.region, case.seed).model.filled_cells(),
        volume,
        "without the air member the same rule fills the box solid"
    );
}

// ---------------------------------------------------------------------------
// 5. Graded erosion
// ---------------------------------------------------------------------------

/// **The gradient is the split.** Three bands, three mixes, and the air share
/// climbs band by band.
#[test]
fn graded_erosion_each_band_is_more_ruined_than_the_one_below() {
    let case = &CASES[4];
    let out = expand_case(case);
    let bands = [(0..4, "sound"), (4..8, "weathered"), (8..13, "ruined")];
    let mut shares = Vec::new();
    for (range, name) in bands {
        let mut total = 0usize;
        let mut air = 0usize;
        for y in range {
            for x in 0..case.region[0] as i32 {
                for z in 0..case.region[2] as i32 {
                    total += 1;
                    if is_air(&out, [x, y, z]) {
                        air += 1;
                    }
                }
            }
        }
        assert!(total > 0, "band {name} bound to nothing");
        shares.push((name, air as f64 / total as f64, total));
    }
    assert_eq!(shares[0].1, 0.0, "the sound band carries no air member");
    assert!(
        shares[0].1 < shares[1].1 && shares[1].1 < shares[2].1,
        "the gradient does not climb: {shares:?}"
    );
    assert_eq!(shares.iter().map(|s| s.2).sum::<usize>(), 351);
}

/// **`rounding` is owed by every surface, not only by floors** — the measured
/// red the reference now states.
///
/// Thirteen courses over three shares do not divide. Under the default
/// `truncate` the pieces are 4, 4, 4 and the thirteenth course is never written:
/// twenty-seven cells of daylight along the top of the wall, and no gate reads
/// it, because `non-empty` and `blocks-exist` are both perfectly happy.
#[test]
fn graded_erosion_a_truncating_band_split_leaves_a_course_unwritten() {
    let case = &CASES[4];
    let top = case.region[1] as i32 - 1;

    let covered = expand_case(case);
    let written = (0..case.region[0] as i32)
        .flat_map(|x| (0..case.region[2] as i32).map(move |z| [x, top, z]))
        .filter(|&c| !is_air(&covered, c))
        .count();
    assert!(written > 0, "the rounded split covers the top course");

    let mut truncating = idioms::graded_erosion();
    set_rounding(&mut truncating, "face", Rounding::Truncate);
    let short = run(&truncating, case.region, case.seed);
    let left = (0..case.region[0] as i32)
        .flat_map(|x| (0..case.region[2] as i32).map(move |z| [x, top, z]))
        .filter(|&c| !is_air(&short, c))
        .count();
    assert_eq!(
        left, 0,
        "the truncating split must leave the top course air"
    );

    // ...and the report is green either way, which is the point of the warning.
    let report = gates::judge(&short, gates::Options::default());
    assert!(report.is_pass(), "{:#?}", report.gates);
}

// ---------------------------------------------------------------------------
// 6. Surface detail
// ---------------------------------------------------------------------------

/// **The rule that built the surface splits off the layer against it.**
///
/// One rule, four pieces: mass, the crust course that is the top of the mass,
/// the litter course standing on the crust, and the air above.
#[test]
fn surface_detail_the_crust_and_the_litter_are_pieces_of_the_ground_rule() {
    let case = &CASES[5];
    let out = expand_case(case);
    let (w, d) = (case.region[0] as i32, case.region[2] as i32);
    let course = |y: i32| -> Vec<String> {
        (0..w)
            .flat_map(|x| (0..d).map(move |z| [x, y, z]))
            .map(|c| block_at(&out, c))
            .collect()
    };

    let rock = course(5);
    assert_eq!(rock.len(), 81);
    assert!(
        rock.iter().all(|b| b == "minecraft:tuff"),
        "the mass is one block"
    );

    let crust = course(6);
    assert!(
        crust.iter().all(|b| b != "minecraft:air"),
        "the crust is the top of the mass, so it is solid"
    );
    assert!(
        crust.iter().collect::<BTreeSet<_>>().len() >= 2,
        "the crust is a mix, not the mass again"
    );

    let litter = course(7);
    assert!(
        litter.iter().any(|b| b == "minecraft:air"),
        "the litter layer is mostly air"
    );
    assert!(
        litter.iter().any(|b| b != "minecraft:air"),
        "...and carries scatter"
    );
    assert!(litter.iter().collect::<BTreeSet<_>>().len() >= 3);

    for y in 8..case.region[1] as i32 {
        assert!(
            course(y).iter().all(|b| b == "minecraft:air"),
            "course {y} should be open air over the litter"
        );
    }
}

// ---------------------------------------------------------------------------
// 7. Symmetry
// ---------------------------------------------------------------------------

fn glazing_cells(out: &Expansion, size: [u32; 3]) -> BTreeSet<[i32; 3]> {
    cells(size)
        .filter(|&c| block_at(out, c) == "minecraft:light_gray_stained_glass")
        .collect()
}

/// **One rule and its reflection give a shape with a mirror plane.**
///
/// The two halves of the aperture are the same rule; the upper one runs under
/// `mirror: {y}`, so it peels its courses off the other end. The aperture that
/// results is symmetric about both centre lines of the wall, asserted cell by
/// cell.
#[test]
fn mirror_one_rule_reflected_gives_a_symmetric_aperture() {
    let case = &CASES[6];
    let size = case.region;
    let out = expand_case(case);
    let glazing = glazing_cells(&out, size);
    assert_eq!(glazing.len(), 114, "57 cells of aperture, two courses deep");

    let (w, h) = (size[0] as i32 - 1, size[1] as i32 - 1);
    for &[x, y, z] in &glazing {
        assert!(
            glazing.contains(&[w - x, y, z]),
            "not symmetric across the vertical centre line at {x},{y},{z}"
        );
        assert!(
            glazing.contains(&[x, h - y, z]),
            "not symmetric across the horizontal centre line at {x},{y},{z}"
        );
    }

    // Course widths: the chamfer, the waist, the chamfer.
    let widths: Vec<usize> = (0..size[1] as i32)
        .map(|y| {
            (0..size[0] as i32)
                .filter(|&x| glazing.contains(&[x, y, 0]))
                .count()
        })
        .collect();
    assert_eq!(widths, vec![0, 3, 5, 7, 9, 9, 9, 7, 5, 3, 0]);
}

/// **Not reflecting it is visible.** Drop the `mirror` and give both halves the
/// rule as written; the aperture stops being symmetric — the same box, the same
/// arithmetic, one node.
#[test]
fn mirror_without_the_reflection_the_aperture_is_lopsided() {
    let case = &CASES[6];
    let mut lopsided = idioms::mirror();
    match &mut lopsided.rules.get_mut("window").expect("rule exists")[0].body {
        Node::Split(split) => split.children[2] = Node::call("half"),
        other => panic!("`window` is not a bare split: {other:?}"),
    }
    let out = run(&lopsided, case.region, case.seed);
    let glazing = glazing_cells(&out, case.region);
    let h = case.region[1] as i32 - 1;
    assert!(
        glazing
            .iter()
            .any(|&[x, y, z]| !glazing.contains(&[x, h - y, z])),
        "an unreflected body must not still be symmetric"
    );
}

/// **The reflection expresses exactly what two hand-kept copies did**, and the
/// point of preferring it is that nothing has to keep them in step. The two
/// programs are compared where it counts: byte for byte.
#[test]
fn mirror_the_reflection_is_the_two_copies_it_replaces() {
    let case = &CASES[6];
    // The upper half, written out: the same splits with their size lists
    // reversed and their children swapped, one rule per recursion level.
    let two_copies = idioms::mirror()
        .rule_alts(
            "upper_half",
            vec![
                Alternative::new(Node::Split(Split {
                    axis: Axis::Y,
                    sizes: vec![Size::abs(1), Size::rel(1)],
                    rounding: Rounding::Start,
                    repeat: false,
                    orient: Reorient::KEEP,
                    children: vec![Node::call("slot"), Node::call("upper_inset")],
                }))
                .when(Cond::All {
                    of: vec![
                        Cond::cmp(Expr::dim(DimRef::X), CmpOp::Ge, Expr::int(3)),
                        Cond::cmp(Expr::dim(DimRef::Y), CmpOp::Ge, Expr::int(2)),
                    ],
                }),
                Alternative::new(Node::call("slot")).when(Cond::Otherwise),
            ],
        )
        .rule(
            "upper_inset",
            Node::Split(Split {
                axis: Axis::X,
                sizes: vec![Size::abs(1), Size::rel(1), Size::abs(1)],
                rounding: Rounding::Start,
                repeat: false,
                orient: Reorient::KEEP,
                children: vec![
                    Node::fill("mass"),
                    Node::call("upper_half"),
                    Node::fill("mass"),
                ],
            }),
        );
    let mut two_copies = two_copies;
    match &mut two_copies.rules.get_mut("window").expect("rule exists")[0].body {
        Node::Split(split) => split.children[2] = Node::call("upper_half"),
        other => panic!("`window` is not a bare split: {other:?}"),
    }

    assert_eq!(
        run(&idioms::mirror(), case.region, case.seed)
            .model
            .canonical_bytes(),
        run(&two_copies, case.region, case.seed)
            .model
            .canonical_bytes(),
        "the reflection and the hand-written copies are the same building"
    );
}

/// **The aperture re-centres itself as the wall widens.** Nothing in the program
/// says where the window goes; a `[margin, aperture, margin]` split says it.
#[test]
fn mirror_the_aperture_re_centres_in_a_wider_wall() {
    let program = idioms::mirror();
    for width in [15u32, 17, 21] {
        let size = [width, 11, 2];
        let out = run(&program, size, 1);
        let glazing = glazing_cells(&out, size);
        assert!(!glazing.is_empty(), "no aperture at width {width}");
        let lo = glazing.iter().map(|c| c[0]).min().unwrap();
        let hi = glazing.iter().map(|c| c[0]).max().unwrap();
        assert_eq!(
            lo + hi,
            width as i32 - 1,
            "the aperture is off centre at width {width} ({lo}..{hi})"
        );
    }
}

// ---------------------------------------------------------------------------
// 8. Skip
// ---------------------------------------------------------------------------

/// **`skip` leaves the box alone, and today that is indistinguishable from
/// `void`.**
///
/// Not an opinion about this example: nothing in the IR writes a cell twice — a
/// split's children partition their box, a rule body is a single node, there is
/// no sequencing operator — so there is never an earlier fill for `skip` to
/// leave standing. Swapping the two moves no byte, which is the honest form of
/// the claim `grammar.md` §2c makes.
#[test]
fn skip_and_void_are_the_same_model_because_nothing_writes_a_cell_twice() {
    let case = &CASES[7];
    let out = expand_case(case);

    let mut bore = 0usize;
    for x in 1..6 {
        for y in 1..4 {
            for z in 0..5 {
                assert!(
                    is_air(&out, [x, y, z]),
                    "the bore is not clear at {x},{y},{z}"
                );
                bore += 1;
            }
        }
    }
    assert_eq!(bore, 75, "5 x 3 x 5 of bore");

    let mut voided = idioms::skip();
    let alts = voided.rules.get_mut("bore").unwrap();
    if let Node::Split(split) = &mut alts[0].body {
        assert_eq!(split.children[1], Node::Skip);
        split.children[1] = Node::Void;
    } else {
        panic!("the `bore` rule is a split");
    }
    assert_eq!(
        run(&voided, case.region, case.seed).model.canonical_bytes(),
        out.model.canonical_bytes(),
        "`skip` and `void` must produce the same bytes today"
    );
}

// ---------------------------------------------------------------------------
// 9. Light
// ---------------------------------------------------------------------------

/// **A lamp is a role, and the sconce rhythm is a split.** The period is a real
/// control: widen it and there are fewer sconces, in the same gallery.
#[test]
fn light_the_sconce_period_is_a_control_over_a_real_rhythm() {
    let case = &CASES[8];
    let program = idioms::light();
    let lamps = |p: &Program| -> Vec<[i32; 3]> {
        let out = run(p, case.region, case.seed);
        cells(case.region)
            .filter(|&c| block_at(&out, c) == "minecraft:sea_lantern")
            .collect()
    };

    let default = lamps(&program);
    assert_eq!(default.len(), 8, "four sconces on each of two walls");
    assert!(
        default.iter().all(|c| c[1] == 2),
        "every sconce is in the one course the rule split off"
    );
    let walls: BTreeSet<i32> = default.iter().map(|c| c[0]).collect();
    assert_eq!(walls, BTreeSet::from([0, 4]), "both walls are lit");
    let spacing: BTreeSet<i32> = default.iter().map(|c| c[2]).collect();
    assert_eq!(spacing, BTreeSet::from([0, 4, 8, 12]));

    let mut sparse = idioms::light();
    sparse.set_param("sconce_period", 6).unwrap();
    assert_eq!(lamps(&sparse).len(), 6, "a longer period is fewer sconces");
}

// ---------------------------------------------------------------------------
// 10. Arguments
// ---------------------------------------------------------------------------

/// **One rule, four contents.** The row's claim, at the documented region.
///
/// Three rules build four heads that differ in paint and in axis, and the four
/// are congruent: the glazing occupies exactly the cells the air does in the
/// head beside it. What `tests/arguments.rs` adds is the other half — that the
/// nine-rule program this replaces is byte-identical, and that one of its copies
/// can drift with every gate green.
#[test]
fn arguments_states_one_recursion_and_calls_it_four_ways() {
    let case = &CASES[9];
    let program = idioms::arguments();
    assert_eq!(
        program.rules.len(),
        3,
        "a plan rule and one two-rule recursion"
    );
    let out = expand_case(case);
    let count = |name: &str| {
        cells(case.region)
            .filter(|&c| block_at(&out, c) == name)
            .count()
    };
    let glass = count("minecraft:light_blue_stained_glass");
    let air = count("minecraft:air");
    assert!(glass > 0, "the bound paint reached the blocks");
    assert_eq!(
        glass, air,
        "the two glazed heads occupy the cells the two open heads leave empty"
    );

    // The frame is read three rules below the call that pushed it: `head` fills
    // `opening`, `shoulders` calls `head`, and neither names the glazing.
    assert_eq!(
        block_at(&out, [3, 6, 11]),
        "minecraft:light_blue_stained_glass"
    );
    assert!(is_air(&out, [3, 6, 3]), "its sibling is under no frame");
}

// ---------------------------------------------------------------------------
// The composition demonstration
// ---------------------------------------------------------------------------

/// **The composition is the idioms, together.** Not a new capability — every
/// claim below is one of the nine, read off one model.
#[test]
fn the_composition_demonstration_carries_the_idioms_it_names() {
    let case = &CASES[10];
    let size = case.region;
    let out = expand_case(case);

    // A composition is the level at which a campaign has something to bind to.
    assert_eq!(
        out.anchors.keys().collect::<Vec<_>>(),
        vec!["anchor/arcade-walk"]
    );

    // Light: a sconce on each face of every pier, all in the one course the
    // pier rule split off — and their `z` positions are where the piers are.
    let lamps: Vec<[i32; 3]> = cells(size)
        .filter(|&c| block_at(&out, c) == "minecraft:sea_lantern")
        .collect();
    assert_eq!(lamps.len(), 12, "two faces x two courses of pier x 3 piers");
    assert!(lamps.iter().all(|c| c[1] == 4), "one sconce course");
    assert_eq!(
        lamps.iter().map(|c| c[0]).collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 2]),
        "both faces of the arcade"
    );

    // Repetition + priority: three piers over the length, the last one placed by
    // the `otherwise` arm when the remainder is too short for another bay.
    assert_eq!(
        lamps.iter().map(|c| c[2]).collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 1, 9, 10, 18, 19]),
        "three piers of two, on the `pier + bay` rhythm"
    );

    // Shape, with the paint inverted: the bay's opening narrows as it climbs.
    // Measured as the longest open run across the bay, so an eroded voussoir
    // cell cannot be mistaken for the opening.
    let opening = |y: i32| -> usize {
        let (mut best, mut run) = (0usize, 0usize);
        for z in 2..9 {
            if is_air(&out, [1, y, z]) {
                run += 1;
                best = best.max(run);
            } else {
                run = 0;
            }
        }
        best
    };
    assert_eq!(opening(3), 7, "the jamb runs full width to the springing");
    assert_eq!(opening(4), 7, "the springing course");
    assert_eq!(opening(5), 5, "one cell in on each side");
    assert_eq!(opening(6), 3, "and again");
    assert_eq!(opening(7), 0, "and the head closes");

    // Erosion, graded up the elevation: the crest carries more air than the
    // wall, which carries more than the footing.
    let air_share = |lo: i32, hi: i32| {
        let (mut air, mut total) = (0usize, 0usize);
        for y in lo..hi {
            for x in 0..size[0] as i32 {
                total += 1;
                if is_air(&out, [x, y, 0]) {
                    air += 1;
                }
            }
        }
        (air as f64 / total as f64, total)
    };
    let (footing, footing_cells) = air_share(0, 1);
    let (wall, _) = air_share(1, 11);
    let (crest, _) = air_share(11, 13);
    assert!(footing_cells > 0);
    assert!(
        footing <= wall && wall < crest,
        "the elevation does not decay upward: {footing} {wall} {crest}"
    );

    // Surface detail: the litter course stands on the crest and is mostly air.
    let litter: Vec<String> = (0..size[0] as i32)
        .flat_map(|x| (0..size[2] as i32).map(move |z| [x, 13, z]))
        .map(|c| block_at(&out, c))
        .collect();
    assert!(litter.iter().any(|b| b == "minecraft:moss_carpet"));
    assert!(litter.iter().filter(|b| *b == "minecraft:air").count() > litter.len() / 2);
}

// ---------------------------------------------------------------------------
// A corpus example that is not an idiom-index entry (spec-0033 §4.8)
// ---------------------------------------------------------------------------

/// **`none_of` holds when no sub-guard does**, which is the complement of
/// `any_of` and the shape of a sentence starting with *unless*.
///
/// `negated_guard` is in the corpus because every IR construct owes
/// `delve-grammar list` an example; it is **not** in the idiom index, because
/// negating a guard is a language feature and not a way of building anything
/// (spec-0033 §4.8). The claim asserted here is only that the guard means what
/// it says, from both sides.
#[test]
fn negated_guard_holds_exactly_when_no_sub_guard_does() {
    let program = library::negated_guard::negated_guard();
    let buttress = "minecraft:polished_andesite";
    let count = |size: [u32; 3]| {
        let out = run(&program, size, 1);
        cells(size)
            .filter(|&c| block_at(&out, c) == buttress)
            .count()
    };

    // Neither disqualification holds: the buttressed arm fires.
    assert_eq!(count([5, 4, 12]), 40, "two buttress courses of 5 x 4");
    // Too thin, and then too short: one sub-guard holding is enough to bar it.
    assert_eq!(count([2, 4, 12]), 0, "`X < min_thick` disqualifies");
    assert_eq!(count([5, 4, 8]), 0, "`Z < min_run` disqualifies");
    // ...and the disqualified box is a plain pier rather than a refusal.
    assert_eq!(run(&program, [2, 4, 12], 1).model.filled_cells(), 96);
}

// ---------------------------------------------------------------------------
// Four facts about the IR that `grammar.md` §2 now states
// ---------------------------------------------------------------------------

/// **`rounding` other than `truncate` is legal on a split with exactly one
/// relative piece — and at weight 1 it changes nothing**, because the remainder
/// of dividing by one is always zero.
///
/// Both halves matter to an author: the first because `RoundingWithoutRelative`
/// refuses only a split with *no* relative piece, the second because reaching
/// for `"rounding": "start"` on `[abs, rel, abs]` is a no-op and the coverage it
/// looks like it is buying has to come from somewhere else.
#[test]
fn fact_rounding_on_one_relative_piece_is_legal_and_inert_at_weight_one() {
    let one_share = |rounding: Rounding| -> Program {
        Program::new("one_share", "band")
            .role("mass", BlockState::simple("stone_bricks"))
            .rule(
                "band",
                Node::Split(Split {
                    axis: Axis::X,
                    sizes: vec![Size::abs(1), Size::rel(1), Size::abs(1)],
                    rounding,
                    repeat: false,
                    orient: Reorient::KEEP,
                    children: vec![Node::fill("mass"), Node::Void, Node::fill("mass")],
                }),
            )
    };
    one_share(Rounding::Start).validate().unwrap();
    for size in [[8u32, 2, 2], [9, 2, 2], [10, 2, 2]] {
        assert_eq!(
            run(&one_share(Rounding::Start), size, 1)
                .model
                .canonical_bytes(),
            run(&one_share(Rounding::Truncate), size, 1)
                .model
                .canonical_bytes(),
            "one share of weight 1 always covers exactly, at {size:?}"
        );
    }

    // At weight 3 there is a remainder to place, and rounding is a real control.
    let three_shares = |rounding: Rounding| -> Program {
        Program::new("three_shares", "band")
            .role("mass", BlockState::simple("stone_bricks"))
            .rule(
                "band",
                Node::Split(Split {
                    axis: Axis::X,
                    sizes: vec![Size::rel(3), Size::abs(1)],
                    rounding,
                    repeat: false,
                    orient: Reorient::KEEP,
                    children: vec![Node::Void, Node::fill("mass")],
                }),
            )
    };
    let filled = |rounding: Rounding| {
        run(&three_shares(rounding), [9, 1, 1], 1)
            .model
            .filled_cells()
    };
    // Truncate: 8 leftover over weight 3 gives 2 per unit, so the pattern covers
    // 6 + 1 of 9 and the last two columns are never written.
    assert_eq!(filled(Rounding::Truncate), 1);
    assert_eq!(filled(Rounding::Start), 1);
    assert_ne!(
        run(&three_shares(Rounding::Start), [9, 1, 1], 1)
            .model
            .canonical_bytes(),
        run(&three_shares(Rounding::Truncate), [9, 1, 1], 1)
            .model
            .canonical_bytes(),
        "at weight 3 the remainder has somewhere to go, so the split moves"
    );
}

/// **`smallest` and `largest` break a tie toward the lowest WORLD axis** — `X`,
/// then `Y`, then `Z` — measured over the axes still free when the extremal
/// spec is resolved. On a cube, `x: largest` therefore names world `X`.
///
/// The same two names read as an *expression* (`{"expr":"dim","dim":"smallest"}`)
/// are a number, not an axis: the smallest of the three world extents, with no
/// tie to break.
#[test]
fn fact_smallest_and_largest_break_a_tie_toward_the_lowest_world_axis() {
    // A rule that voids a one-block slab off whichever axis `largest` picks.
    let probe = |spec: AxisSpec| -> Program {
        Program::new("tie", "mark_axis")
            .role("mass", BlockState::simple("stone_bricks"))
            .rule(
                "mark_axis",
                Node::Reorient {
                    orient: Reorient::KEEP.x(spec),
                    body: Box::new(Node::Split(Split {
                        axis: Axis::X,
                        sizes: vec![Size::abs(1), Size::rel(1)],
                        rounding: Rounding::Truncate,
                        repeat: false,
                        orient: Reorient::KEEP,
                        children: vec![Node::Void, Node::fill("mass")],
                    })),
                },
            )
    };
    // A cube: all three extents tie.
    let out = run(&probe(AxisSpec::Largest), [5, 5, 5], 1);
    assert!(
        is_air(&out, [0, 2, 2]) && !is_air(&out, [2, 0, 2]) && !is_air(&out, [2, 2, 0]),
        "a tie must resolve to world X"
    );
    let out = run(&probe(AxisSpec::Smallest), [5, 5, 5], 1);
    assert!(is_air(&out, [0, 2, 2]), "and so must the other extremal");
    // Not a tie: `largest` genuinely follows the box.
    let out = run(&probe(AxisSpec::Largest), [5, 5, 9], 1);
    assert!(
        is_air(&out, [2, 2, 0]) && !is_air(&out, [0, 2, 2]),
        "world Z is the long axis here"
    );
}

/// **A relative piece that resolves to zero blocks is a silent empty child, not
/// an error.**
///
/// A `Relative` *weight* of zero is refused (`BadSize`) — but a positive weight
/// with nothing left to share is a legal, zero-volume scope. `fill`, `void` and
/// `skip` all write nothing in it and the expansion carries on, so a rule that
/// depends on the piece existing has no diagnostic to lean on. What does refuse
/// is anything that needs a cell: an absolute split inside it overflows, and a
/// `mark` on it is `MarkOutsideScope`.
#[test]
fn fact_a_zero_length_relative_piece_is_a_silent_empty_child() {
    let squeezed = |body: Node| -> Program {
        Program::new("squeezed", "band")
            .role("mass", BlockState::simple("stone_bricks"))
            .rule(
                "band",
                Node::Split(Split {
                    axis: Axis::X,
                    // Two absolute cells and nothing left over for the share.
                    sizes: vec![Size::abs(1), Size::rel(1), Size::abs(1)],
                    rounding: Rounding::Start,
                    repeat: false,
                    orient: Reorient::KEEP,
                    children: vec![Node::fill("mass"), body, Node::fill("mass")],
                }),
            )
    };
    let out = run(&squeezed(Node::fill("mass")), [2, 2, 2], 1);
    assert_eq!(
        out.model.filled_cells(),
        8,
        "both walls, and nothing between"
    );

    // The same empty scope refuses the moment something needs a cell of it.
    let err = expand(
        &squeezed(Node::Split(Split {
            axis: Axis::X,
            sizes: vec![Size::abs(1)],
            rounding: Rounding::Truncate,
            repeat: false,
            orient: Reorient::KEEP,
            children: vec![Node::fill("mass")],
        })),
        Box3::at_origin([2, 2, 2]),
        &ExpandOptions::seeded(1),
    )
    .unwrap_err();
    assert!(matches!(err, ExpandError::Split { .. }), "{err}");

    // A zero *weight* is refused before anything expands.
    let bad = Program::new("zero_weight", "band")
        .role("mass", BlockState::simple("stone_bricks"))
        .rule(
            "band",
            Node::Split(Split {
                axis: Axis::X,
                sizes: vec![Size::rel(0)],
                rounding: Rounding::Truncate,
                repeat: false,
                orient: Reorient::KEEP,
                children: vec![Node::fill("mass")],
            }),
        );
    let err = expand(&bad, Box3::at_origin([4, 2, 2]), &ExpandOptions::seeded(1)).unwrap_err();
    assert!(
        matches!(err, ExpandError::BadSize { value: 0, .. }),
        "{err}"
    );
}

/// **A role bound to a world-cardinal block state does not turn when `largest`
/// turns the scope.**
///
/// A `fill` writes the block state it was given, verbatim; nothing rotates a
/// `facing=` property to follow the scope's orientation. So a rule whose frame
/// opens with `z(Largest)` — which every staging rule in this library does —
/// lays its stairs, its doors and its voussoirs the same way round whatever box
/// it is handed, and every gate stays green while the piece faces the wrong way.
///
/// The construct that answers it is the `orientation` guard: one alternative per
/// axis mapping, each naming the block state that mapping wants, which is how
/// `church` picks its four roof stair facings.
#[test]
fn fact_a_world_cardinal_state_does_not_turn_with_the_scope() {
    let program = Program::new("facing", "piece")
        .role(
            "tread",
            BlockState::with("stone_brick_stairs", [("facing", "east")]),
        )
        .rule(
            "piece",
            Node::Reorient {
                orient: Reorient::KEEP.z(AxisSpec::Largest),
                body: Box::new(Node::fill("tread")),
            },
        );

    // Two boxes whose long horizontal axis is different, so `largest` gives the
    // rule two different orientations.
    for size in [[9u32, 1, 3], [3, 1, 9]] {
        let out = run(&program, size, 1);
        assert!(
            out.model
                .palette()
                .iter()
                .any(|b| b.properties.get("facing").map(String::as_str) == Some("east")),
            "the stair still faces east at {size:?}"
        );
        assert_eq!(
            out.model.palette().len(),
            2,
            "one state, whichever way the scope turned"
        );
    }

    // The guard that CAN answer it: an alternative per axis mapping.
    let guarded = Program::new("guarded_facing", "piece")
        .role(
            "tread_x",
            BlockState::with("stone_brick_stairs", [("facing", "east")]),
        )
        .role(
            "tread_z",
            BlockState::with("stone_brick_stairs", [("facing", "south")]),
        )
        .rule_alts(
            "piece",
            vec![
                Alternative::new(Node::fill("tread_x")).when(Cond::orientation(
                    Axis::X,
                    Axis::Y,
                    Axis::Z,
                )),
                Alternative::new(Node::fill("tread_z")).when(Cond::Otherwise),
            ],
        );
    let straight = run(&guarded, [4, 1, 1], 1);
    assert_eq!(
        straight.model.palette()[1].properties["facing"],
        "east",
        "the identity orientation picks its own state"
    );
    let turned = expand(
        &guarded,
        Box3::at_origin([4, 1, 1]),
        &ExpandOptions {
            seed: 1,
            limits: Default::default(),
            orientation: delvewright_grammar::geom::Orientation::from_axes([
                Axis::Z,
                Axis::Y,
                Axis::X,
            ]),
        },
    )
    .unwrap();
    assert_eq!(turned.model.palette()[1].properties["facing"], "south");
}

// ---------------------------------------------------------------------------
// Small surgery on a program, for the red demonstrations
// ---------------------------------------------------------------------------

/// Drop the `otherwise` alternative of one rule.
fn strip_otherwise(program: &mut Program, symbol: &str) {
    let alts = program.rules.get_mut(symbol).expect("rule exists");
    let before = alts.len();
    alts.retain(|a: &Alternative| !matches!(a.when, Cond::Otherwise));
    assert_eq!(before, alts.len() + 1, "{symbol} had one `otherwise` arm");
}

/// Re-round the split that is one rule's whole body.
fn set_rounding(program: &mut Program, symbol: &str, rounding: Rounding) {
    let alts = program.rules.get_mut(symbol).expect("rule exists");
    match &mut alts[0].body {
        Node::Split(split) => split.rounding = rounding,
        other => panic!("{symbol} is not a bare split: {other:?}"),
    }
}

/// Keep `Material` in scope for the doc examples above without an unused import.
#[allow(dead_code)]
fn _material_is_part_of_the_surface(block: BlockState) -> Material {
    Material::block(block)
}
