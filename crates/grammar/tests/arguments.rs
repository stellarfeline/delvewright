//! **A call's callee reads the frame it is called under** — `bind`, the
//! construct that lets one rule be called with different content.
//!
//! Trial 0001 named this the language's one clearly missing primitive, twice,
//! with a cost that grew with scale: 29 of 113 rules in run 0 were byte-identical
//! to another once role names and call targets were erased, 44 of 145 in run 1.
//! Its worst single case is the one this file is built on — **one pointed-arch
//! recursion written four times**, because neither the paint nor the axis could
//! be chosen by the caller.
//!
//! Four things are proved here, and the order is the order they would be
//! expensive to discover false in:
//!
//! 1. **The collapse is exact.** The four-copy program and `idiom-arguments` are
//!    byte-identical at every seed. Nine rules become three and not one cell
//!    moves.
//! 2. **The copies are a silent-defect generator, and the collapse is not.** One
//!    copy edited out of step passes `validate`, both expansion gates with
//!    non-zero bindings, the determinism gate and the coverage report, and the
//!    four heads are different shapes. The same intended edit made once on the
//!    collapsed program moves all four together.
//! 3. **A binding moves no block anywhere else.** Every alternative of every
//!    library program is wrapped in a frame that rebinds every name to itself,
//!    at three seeds: same blocks, same anchors.
//! 4. **Determinism survives it**, in separate processes, over a program whose
//!    one rule is reached under four different frames.
//!
//! Plus what the scoping rule is, what stops a changing argument from diverging,
//! and what the construct refuses.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use delvewright_grammar::block::BlockState;
use delvewright_grammar::coverage;
use delvewright_grammar::gates;
use delvewright_grammar::geom::Axis;
use delvewright_grammar::ir::{
    Alternative, ArithOp, CmpOp, Cond, DimRef, Expr, Material, Node, Program, ProgramError,
    Reorient, Rounding, Size, Split,
};
use delvewright_grammar::library;
use delvewright_grammar::{Box3, ExpandError, ExpandOptions, Expansion, Limits, expand};

const GRAMMAR: &str = env!("CARGO_BIN_EXE_delve-grammar");

/// The region `idiom-arguments` is documented at: four 7 × 7 × 7 quadrants
/// around a one-cell cross of masonry.
const REGION: [u32; 3] = [15, 7, 15];

fn run(program: &Program, size: [u32; 3], seed: u64) -> Expansion {
    expand(program, Box3::at_origin(size), &ExpandOptions::seeded(seed))
        .unwrap_or_else(|e| panic!("{}: {e}", program.name))
}

// ---------------------------------------------------------------------------
// The program `idiom-arguments` replaces
// ---------------------------------------------------------------------------

fn abs(n: i64) -> Size {
    Size::abs(n)
}

fn abse(blocks: Expr) -> Size {
    Size::Absolute { blocks }
}

fn rel(w: i64) -> Size {
    Size::rel(w)
}

fn split_exact(axis: Axis, sizes: Vec<Size>, children: Vec<Node>) -> Node {
    Node::Split(Split {
        axis,
        sizes,
        rounding: Rounding::Start,
        repeat: false,
        orient: Reorient::KEEP,
        children,
    })
}

/// One copy of the pointed-arch recursion: two rules, fixed at authoring time to
/// one axis and one paint.
///
/// It is generated here rather than typed out four times because what is under
/// test is the **program**, not how a test wrote it — expansion cannot tell the
/// difference, and an author who reached for a generator to avoid typing it four
/// times would be reaching for exactly the workaround the trial calls worse than
/// the duplication (the artifact of record stops being the artifact the tool
/// consumes).
fn copy(suffix: &str, axis: Axis, dim: DimRef, paint: &str, inset: i64) -> Vec<(String, Node)> {
    let step = || {
        Expr::int(inset).arith(
            ArithOp::Max,
            Expr::dim(dim).arith(ArithOp::Div, Expr::param("run")),
        )
    };
    let head = format!("head{suffix}");
    let shoulders = format!("shoulders{suffix}");
    vec![
        (head.clone(), {
            Node::Split(Split {
                axis: Axis::Y,
                sizes: vec![abs(1), rel(1)],
                rounding: Rounding::Start,
                repeat: false,
                orient: Reorient::KEEP,
                children: vec![Node::fill(paint), Node::call(&shoulders)],
            })
        }),
        (
            shoulders,
            split_exact(
                axis,
                vec![abse(step()), rel(1), abse(step())],
                vec![Node::fill("mass"), Node::call(&head), Node::fill("mass")],
            ),
        ),
    ]
}

/// The guard is the same in every copy, so it is written once here; in a
/// hand-written four-copy program it is written four times too.
fn head_guard(dim: DimRef, inset: i64) -> Cond {
    let step = Expr::int(inset).arith(
        ArithOp::Max,
        Expr::dim(dim).arith(ArithOp::Div, Expr::param("run")),
    );
    Cond::All {
        of: vec![
            Cond::cmp(
                Expr::dim(dim),
                CmpOp::Ge,
                step.arith(ArithOp::Mul, Expr::int(2))
                    .arith(ArithOp::Add, Expr::int(1)),
            ),
            Cond::cmp(Expr::dim(DimRef::Y), CmpOp::Ge, Expr::int(2)),
        ],
    }
}

/// **Four copies of one recursion**, which is what the trial's program carries
/// and what `idiom-arguments` collapses.
///
/// `insets` is the per-copy taper step, in the order the quadrants are laid out
/// (open-X, glazed-X, open-Z, glazed-Z). All four are `1` in the faithful copy;
/// changing one is how a copy goes out of step.
fn four_copies(insets: [i64; 4]) -> Program {
    let specs: [(&str, Axis, DimRef, &str); 4] = [
        ("_open_x", Axis::X, DimRef::X, "opening"),
        ("_glazed_x", Axis::X, DimRef::X, "glazing"),
        ("_open_z", Axis::Z, DimRef::Z, "opening"),
        ("_glazed_z", Axis::Z, DimRef::Z, "glazing"),
    ];
    let mut program = Program::new("four_copies", "piece")
        .param("run", 6)
        .role("mass", BlockState::simple("stone_bricks"))
        .role("opening", BlockState::air())
        .role("glazing", BlockState::simple("light_blue_stained_glass"))
        .rule(
            "piece",
            split_exact(
                Axis::X,
                vec![rel(1), abs(1), rel(1)],
                vec![
                    split_exact(
                        Axis::Z,
                        vec![rel(1), abs(1), rel(1)],
                        vec![
                            Node::call("head_open_x"),
                            Node::fill("mass"),
                            Node::call("head_glazed_x"),
                        ],
                    ),
                    Node::fill("mass"),
                    split_exact(
                        Axis::Z,
                        vec![rel(1), abs(1), rel(1)],
                        vec![
                            Node::call("head_open_z"),
                            Node::fill("mass"),
                            Node::call("head_glazed_z"),
                        ],
                    ),
                ],
            ),
        );
    for (i, (suffix, axis, dim, paint)) in specs.into_iter().enumerate() {
        let inset = insets[i];
        for (n, (symbol, body)) in copy(suffix, axis, dim, paint, inset)
            .into_iter()
            .enumerate()
        {
            program = if n == 0 {
                program.rule_alts(
                    &symbol,
                    vec![
                        Alternative::new(body).when(head_guard(dim, inset)),
                        Alternative::new(Node::fill(paint)).when(Cond::Otherwise),
                    ],
                )
            } else {
                program.rule(&symbol, body)
            };
        }
    }
    program
}

// ---------------------------------------------------------------------------
// Reading the four heads back off the blocks
// ---------------------------------------------------------------------------

/// The cells of one 7 × 7 × 7 quadrant that are **not** masonry, in local
/// coordinates, transposed when the quadrant's head tapers across `Z`.
///
/// Read off the blocks rather than off the rules, so it says what was built and
/// not what was written.
fn head_shape(out: &Expansion, x0: i32, z0: i32, turned: bool) -> BTreeSet<[i32; 3]> {
    let mut cells = BTreeSet::new();
    for dx in 0..7 {
        for dy in 0..7 {
            for dz in 0..7 {
                let block = out.model.get([x0 + dx, dy, z0 + dz]).unwrap();
                if block.name != "minecraft:stone_bricks" {
                    cells.insert(if turned { [dz, dy, dx] } else { [dx, dy, dz] });
                }
            }
        }
    }
    cells
}

/// All four heads, in the layout order `four_copies` uses.
fn four_heads(out: &Expansion) -> [BTreeSet<[i32; 3]>; 4] {
    [
        head_shape(out, 0, 0, false),
        head_shape(out, 0, 8, false),
        head_shape(out, 8, 0, true),
        head_shape(out, 8, 8, true),
    ]
}

// ---------------------------------------------------------------------------
// 1. The collapse is exact
// ---------------------------------------------------------------------------

/// **Nine rules become three and not one cell moves.**
///
/// This is the acceptance case. `idiom-arguments` states the recursion once and
/// calls it four ways — the paint from a `bind`, the axis from a `reorient` —
/// and is byte-identical to the four-copy program at every seed, anchors
/// included.
#[test]
fn one_rule_called_four_ways_is_the_four_copies_byte_for_byte() {
    let copies = four_copies([1; 4]);
    let collapsed = library::idioms::arguments();
    assert_eq!(
        copies.rules.len(),
        9,
        "one plan rule plus four copies of two"
    );
    assert_eq!(collapsed.rules.len(), 3, "one plan rule and one recursion");

    for seed in [0u64, 1, 7, 4242] {
        let a = run(&copies, REGION, seed);
        let b = run(&collapsed, REGION, seed);
        assert_eq!(
            a.model.canonical_bytes(),
            b.model.canonical_bytes(),
            "seed {seed}"
        );
        assert_eq!(a.anchors, b.anchors, "seed {seed}");
    }

    // …and the collapse is not a collapse onto one *content*: the four heads
    // really are four different things, in two paints and two axes.
    let out = run(&collapsed, REGION, 1);
    assert_eq!(
        out.model
            .palette()
            .iter()
            .filter(|b| b.name == "minecraft:light_blue_stained_glass")
            .count(),
        1,
        "the glazing was bound and reached the blocks"
    );
    let glass = (0..15)
        .flat_map(|x| (0..7).flat_map(move |y| (0..15).map(move |z| [x, y, z])))
        .filter(|c| out.model.get(*c).unwrap().name == "minecraft:light_blue_stained_glass")
        .count();
    let air = (0..15)
        .flat_map(|x| (0..7).flat_map(move |y| (0..15).map(move |z| [x, y, z])))
        .filter(|c| out.model.get(*c).unwrap().is_air())
        .count();
    assert!(glass > 0 && glass == air, "glass {glass}, air {air}");
}

/// **The paint is bound at the call and read three rules deeper**, which is the
/// scoping decision this construct turns on.
///
/// `head` fills `opening`; `shoulders` calls `head`; neither mentions glazing.
/// If a frame did not survive a call, the glazed quadrant would be air and this
/// assertion would find nothing.
#[test]
fn a_binding_is_inherited_through_the_calls_of_a_recursion() {
    let out = run(&library::idioms::arguments(), REGION, 1);
    // The glazed quadrant's apex slot is written by the recursion's `otherwise`
    // arm, four calls below the `bind`.
    assert_eq!(
        out.model.get([3, 6, 11]).unwrap().name,
        "minecraft:light_blue_stained_glass"
    );
    // Its sibling, expanded from the same rule under no frame at all, is air.
    assert!(out.model.get([3, 6, 3]).unwrap().is_air());
}

// ---------------------------------------------------------------------------
// 2. The red: four copies, one edited out of step, every gate green
// ---------------------------------------------------------------------------

/// Every gate this back end has, run over one expansion, with its binding count.
fn gate_report(program: &Program, seed: u64) -> gates::Report {
    gates::judge(
        &run(program, REGION, seed),
        gates::Options {
            traversable: false,
            allow_falls: false,
            symmetric: None,
            reachable_floor: false,
        },
    )
}

/// **The red.** One of four copies is edited and the other three are not.
///
/// `validate` passes, `blocks-exist` and `non-empty` pass with non-zero
/// bindings, the expansion reproduces byte for byte, and the demonstration
/// coverage report passes. Nothing anywhere says that the building now carries
/// two different arches — because nothing can: the four copies were never
/// declared to be the same thing.
#[test]
fn a_copy_edited_out_of_step_is_green_on_every_gate() {
    let drifted = four_copies([1, 1, 2, 1]);
    drifted
        .validate()
        .expect("a drifted copy is a valid program");

    let report = gate_report(&drifted, 1);
    println!("four copies, one edited: verdict {}", report.verdict);
    assert!(report.is_pass(), "{:#?}", report.gates);
    assert_eq!(report.gates.len(), 2);
    for gate in &report.gates {
        println!(
            "  {:<14} {}  bound {:<7} {}",
            gate.id,
            if gate.pass { "pass" } else { "FAIL" },
            gate.bound,
            gate.detail
        );
        assert!(gate.bound > 0, "{} bound nothing: {}", gate.id, gate.detail);
    }

    // Determinism is green too: it is a promise about reproducing the bytes, not
    // about the bytes being the ones anyone meant.
    assert_eq!(
        run(&drifted, REGION, 1).model.canonical_bytes(),
        run(&drifted, REGION, 1).model.canonical_bytes()
    );

    // And the coverage report, over the corpus that now demonstrates `bind`, is
    // a pass: the construct is measured, the drift is not measurable at all.
    assert!(coverage::measure(library::PROGRAMS).is_pass());

    // The finding no gate can state: two of the four heads are now different
    // shapes, and the difference is exactly the edit nobody propagated.
    let heads = four_heads(&run(&drifted, REGION, 1));
    println!(
        "  head cell counts: open-X {} glazed-X {} open-Z {} glazed-Z {}",
        heads[0].len(),
        heads[1].len(),
        heads[2].len(),
        heads[3].len()
    );
    assert_eq!(heads[0], heads[1], "the two X heads still agree");
    assert_eq!(
        heads[0], heads[3],
        "the glazed Z head was not the one edited"
    );
    assert_ne!(
        heads[0], heads[2],
        "the edited copy drifted, and this is the assertion no gate makes"
    );
}

/// **The red, at the construct.** Strip the frames and one rule can only make
/// one content.
///
/// This is the state of the language before `bind`: `idiom-arguments` with its
/// two `bind` nodes removed is a program four calls into one recursion, and all
/// four heads come out of the same paint. The glazing is declared, reachable and
/// never written — no gate has anything to say about it, which is why the only
/// way to get a second content was a second copy of the recursion.
#[test]
fn without_a_frame_one_rule_can_only_build_one_content() {
    fn strip(node: Node) -> Node {
        match node {
            Node::Bind { body, .. } => strip(*body),
            Node::Reorient { orient, body } => Node::Reorient {
                orient,
                body: Box::new(strip(*body)),
            },
            Node::Mark { mark, body } => Node::Mark {
                mark,
                body: Box::new(strip(*body)),
            },
            Node::Split(mut split) => {
                split.children = split.children.into_iter().map(strip).collect();
                Node::Split(split)
            }
            other => other,
        }
    }
    let mut stripped = library::idioms::arguments();
    let rules = std::mem::take(&mut stripped.rules);
    stripped.rules = rules
        .into_iter()
        .map(|(symbol, alts)| {
            let bare = alts
                .into_iter()
                .map(|alt| Alternative {
                    weight: alt.weight,
                    when: alt.when,
                    body: strip(alt.body),
                })
                .collect();
            (symbol, bare)
        })
        .collect();
    stripped.validate().unwrap();

    let out = run(&stripped, REGION, 1);
    let glass = (0..15)
        .flat_map(|x| (0..7).flat_map(move |y| (0..15).map(move |z| [x, y, z])))
        .filter(|c| out.model.get(*c).unwrap().name == "minecraft:light_blue_stained_glass")
        .count();
    println!("without the frames: {glass} glazed cell(s); the role is declared and never written");
    assert_eq!(glass, 0, "one rule, one content");
    assert_ne!(
        out.model.canonical_bytes(),
        run(&four_copies([1; 4]), REGION, 1).model.canonical_bytes(),
        "so it is NOT the four-copy building"
    );

    // With the frames, it is — byte for byte. That difference is the whole
    // construct.
    println!(
        "with the frames:    {} glazed cell(s), and the bytes are the four copies'",
        (0..15)
            .flat_map(|x| (0..7).flat_map(move |y| (0..15).map(move |z| [x, y, z])))
            .filter(|c| run(&library::idioms::arguments(), REGION, 1)
                .model
                .get(*c)
                .unwrap()
                .name
                == "minecraft:light_blue_stained_glass")
            .count()
    );
}

/// **The green.** The same intended change, made once.
///
/// An author who wants a steeper arch edits the one recursion. All four heads
/// move together and stay congruent, because there is only one of them.
#[test]
fn the_collapsed_program_cannot_drift_because_there_is_one_rule() {
    let faithful = run(&library::idioms::arguments(), REGION, 1);
    let before = four_heads(&faithful);
    assert_eq!(before[0], before[1]);
    assert_eq!(before[0], before[2]);
    assert_eq!(before[0], before[3]);

    // The intended change: a steeper taper. One edit, in one place — `run` is
    // the recursion's own control, and there is one recursion.
    let mut steeper = library::idioms::arguments();
    steeper.set_param("run", 3).unwrap();
    let after = four_heads(&run(&steeper, REGION, 1));
    assert_ne!(before[0], after[0], "the edit did something");
    assert_eq!(after[0], after[1], "and it did it to all four");
    assert_eq!(after[0], after[2]);
    assert_eq!(after[0], after[3]);

    // The four-copy program needs the same edit made four times, and gets no
    // help if it is made three: this is the same `run` change applied to one
    // copy only.
    let partial = four_copies([1, 1, 2, 1]);
    let partial_heads = four_heads(&run(&partial, REGION, 1));
    assert_ne!(partial_heads[0], partial_heads[2]);
}

// ---------------------------------------------------------------------------
// 3. A binding moves no block anywhere else — over the whole library
// ---------------------------------------------------------------------------

/// A region every library program expands in, with the id it belongs to.
///
/// Its own table rather than one borrowed from `tests/library.rs`, which covers
/// the buildings and the staging vocabulary and deliberately not the idiom
/// index: the claim below is about **every** program an author can reach, so the
/// table has to be total, and a partial one borrowed from elsewhere would look
/// total.
const CORPUS: &[(&str, [u32; 3])] = &[
    ("ambush-door", [11, 5, 13]),
    ("bait-stand", [9, 8, 14]),
    ("boulder-stair", [9, 6, 27]),
    ("broken-grate", [3, 5, 14]),
    ("castle", [41, 14, 25]),
    ("causeway", [7, 10, 9]),
    ("church", [15, 16, 30]),
    ("cliff-path", [3, 6, 30]),
    ("disarm-stand", [9, 7, 16]),
    ("drop-shaft", [4, 8, 6]),
    ("dumbwaiter", [6, 8, 8]),
    ("elite-ground", [19, 5, 25]),
    ("far-side-bar", [5, 5, 7]),
    ("hearth-ward", [8, 6, 14]),
    ("idiom-arguments", [15, 7, 15]),
    ("idiom-composition-arcade", [3, 14, 20]),
    ("idiom-erosion", [9, 5, 3]),
    ("idiom-erosion-graded", [9, 13, 3]),
    ("idiom-light", [5, 6, 13]),
    ("idiom-mirror", [15, 11, 2]),
    ("idiom-priority", [13, 6, 2]),
    ("idiom-repetition", [3, 5, 17]),
    ("idiom-shape", [15, 9, 3]),
    ("idiom-skip", [7, 5, 5]),
    ("idiom-surface-detail", [9, 12, 9]),
    ("lift-shaft", [5, 16, 7]),
    ("negated-guard", [5, 4, 12]),
    ("rafter-hall", [13, 6, 25]),
    ("spatial-contract", [11, 6, 15]),
    ("stair-flight", [5, 14, 22]),
    ("store-room", [7, 5, 14]),
    ("tee-passage", [5, 5, 12]),
    ("temple", [13, 14, 21]),
    ("threshold-motif", [9, 6, 13]),
    ("watch-bay", [7, 7, 24]),
];

/// The table above is **total**, in both directions.
///
/// Without it a program added to the library would simply not be covered by the
/// transparency claim below, and the claim would read as if it were.
#[test]
fn the_transparency_corpus_covers_every_library_program() {
    let listed: BTreeSet<&str> = CORPUS.iter().map(|(id, _)| *id).collect();
    let registered: BTreeSet<&str> = library::PROGRAMS.iter().map(|(id, _)| *id).collect();
    assert_eq!(listed, registered);
    assert_eq!(listed.len(), CORPUS.len(), "the table repeats an id");
}

/// Wrap every alternative of every rule in a frame that rebinds **every name the
/// program declares, to itself**.
///
/// A mechanical transformation rather than a hand-written pair: what is being
/// ruled out is a frame that is inert only where somebody put one.
fn bind_everything(mut program: Program) -> Program {
    let params: BTreeMap<String, Expr> = program
        .params
        .keys()
        .map(|name| (name.clone(), Expr::param(name)))
        .collect();
    let palette: BTreeMap<String, Material> = program
        .palette
        .keys()
        .map(|role| (role.clone(), Material::role(role)))
        .collect();
    let rules = std::mem::take(&mut program.rules);
    program.rules = rules
        .into_iter()
        .map(|(symbol, alts)| {
            let wrapped = alts
                .into_iter()
                .map(|alt| Alternative {
                    weight: alt.weight,
                    when: alt.when,
                    body: Node::Bind {
                        params: params.clone(),
                        palette: palette.clone(),
                        body: Box::new(alt.body),
                    },
                })
                .collect();
            (symbol, wrapped)
        })
        .collect();
    program
}

/// Count the `bind` nodes a program writes, so "the wrapper was there" is a
/// measurement rather than an assumption.
fn binds(program: &Program) -> usize {
    fn walk(node: &Node) -> usize {
        match node {
            Node::Bind { body, .. } => 1 + walk(body),
            Node::Reorient { body, .. } | Node::Mark { body, .. } | Node::Claim { body, .. } => {
                walk(body)
            }
            Node::Split(split) => split.children.iter().map(walk).sum(),
            Node::Void | Node::Skip | Node::Fill { .. } | Node::Call { .. } => 0,
        }
    }
    program
        .rules
        .values()
        .flat_map(|alts| alts.iter())
        .map(|alt| walk(&alt.body))
        .sum()
}

/// **Rebinding every name to itself moves no block in any library program.**
///
/// Every program, every alternative of every rule wrapped, at three seeds: same
/// blocks, same anchors. Stated over the corpus rather than over an example,
/// because a wrapper that was inert in the one place it was tested is exactly
/// the failure this exists to rule out.
#[test]
fn an_identity_frame_moves_no_block_in_any_library_program() {
    let mut checked = 0;
    let mut wrapped_nodes = 0;
    for (id, size) in CORPUS {
        let program = library::by_id(id).unwrap_or_else(|| panic!("{id} is not in the library"));
        assert!(
            !program.params.is_empty() || !program.palette.is_empty(),
            "{id} declares no name to rebind, so the wrap below would be empty"
        );
        let bound = bind_everything(program.clone());
        bound
            .validate()
            .unwrap_or_else(|e| panic!("{id}: the wrapped program is invalid: {e}"));
        let count = binds(&bound);
        assert!(
            count > 0,
            "{id}: nothing was wrapped, so nothing was proved"
        );
        wrapped_nodes += count;
        for seed in [0u64, 1, 7] {
            let plain = run(&program, *size, seed);
            let wrapped = run(&bound, *size, seed);
            assert_eq!(
                plain.model.canonical_bytes(),
                wrapped.model.canonical_bytes(),
                "{id} at seed {seed}: a binding moved a block"
            );
            assert_eq!(plain.anchors, wrapped.anchors, "{id} at seed {seed}");
            checked += 1;
        }
    }
    assert_eq!(checked, CORPUS.len() * 3);
    println!(
        "identity frames wrapped: {wrapped_nodes} across {} programs",
        CORPUS.len()
    );
}

/// The other direction, so the test above is not green because a `bind` does
/// nothing at all: a **non**-identity frame over the same wrap changes the
/// bytes, in every program that binds a role.
#[test]
fn a_non_identity_frame_does_change_the_bytes() {
    let mut moved = 0;
    let mut examined = 0;
    for (id, size) in CORPUS {
        let program = library::by_id(id).unwrap();
        // A role bound to one solid block, so a rebinding to another solid block
        // is a pure restyle: rebinding a role that carries air would change the
        // filled count for a reason that has nothing to do with the frame.
        let Some(role) = program
            .palette
            .iter()
            .find(|(_, paint)| {
                matches!(paint, delvewright_grammar::ir::Paint::Block(b) if !b.is_air())
            })
            .map(|(role, _)| role.clone())
        else {
            continue;
        };
        examined += 1;
        let mut recoloured = program.clone();
        let start = recoloured.start.clone();
        let alts = recoloured.rules.remove(&start).unwrap();
        recoloured.rules.insert(
            start,
            alts.into_iter()
                .map(|alt| Alternative {
                    weight: alt.weight,
                    when: alt.when,
                    body: alt.body.with_roles([(
                        role.as_str(),
                        Material::block(BlockState::simple("purpur_block")),
                    )]),
                })
                .collect(),
        );
        recoloured
            .validate()
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        let a = run(&program, *size, 1);
        let b = run(&recoloured, *size, 1);
        assert_eq!(
            a.model.filled_cells(),
            b.model.filled_cells(),
            "{id}: a rebinding moved geometry, not paint"
        );
        assert_eq!(a.anchors, b.anchors, "{id}");
        if a.model.canonical_bytes() != b.model.canonical_bytes() {
            moved += 1;
        }
    }
    assert!(examined >= 30, "only {examined} programs were examined");
    assert!(
        moved >= examined - 2,
        "only {moved} of {examined} programs changed under a rebound role — the frame is not \
         reaching the fills it should"
    );
}

// ---------------------------------------------------------------------------
// 4. Determinism, in separate processes
// ---------------------------------------------------------------------------

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-grammar-args-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cli_expand(args: &[&str], out: &Path) {
    let result = Command::new(GRAMMAR)
        .arg("expand")
        .args(args)
        .args(["--region", "15x7x15", "--seed", "5", "--id", "piece", "-o"])
        .arg(out)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

/// **Two processes, the same bytes**, over the program whose single rule is
/// reached under four different frames.
///
/// A frame is resolved from `BTreeMap`s and draws nothing from the RNG, so it
/// can perturb neither the draw order nor the visit order — but that is an
/// argument, and ADR-0006 is a measurement.
#[test]
fn the_same_program_under_four_frames_expands_identically_in_two_processes() {
    let a = scratch("a");
    let b = scratch("b");
    cli_expand(&["--program", "idiom-arguments"], &a);
    cli_expand(&["--program", "idiom-arguments"], &b);
    for name in ["piece.nbt", "piece.json"] {
        assert_eq!(
            std::fs::read(a.join(name)).unwrap(),
            std::fs::read(b.join(name)).unwrap(),
            "{name} differs between two processes"
        );
    }

    // And the four-copy program, expanded through the JSON authoring form,
    // reaches the same `.nbt` — the collapse holds through the tool, not only
    // through the library.
    let json = scratch("json");
    let file = json.join("four_copies.json");
    std::fs::write(&file, serde_json::to_string(&four_copies([1; 4])).unwrap()).unwrap();
    let c = scratch("c");
    cli_expand(&["--file", file.to_str().unwrap()], &c);
    assert_eq!(
        std::fs::read(a.join("piece.nbt")).unwrap(),
        std::fs::read(c.join("piece.nbt")).unwrap()
    );
}

// ---------------------------------------------------------------------------
// The scoping rule
// ---------------------------------------------------------------------------

/// A one-cell program whose block name reports which paint `wall` resolves to.
fn probe(body: Node) -> Program {
    Program::new("probe", "start")
        .param("n", 1)
        .role("wall", BlockState::simple("stone"))
        .role("other", BlockState::simple("glass"))
        .rule("start", body)
        .rule("leaf", Node::fill("wall"))
}

/// **A frame lasts exactly as long as its body**, and nothing outlives it.
#[test]
fn a_frame_has_the_extent_of_its_body_and_no_more() {
    let program = probe(Node::Split(Split {
        axis: Axis::X,
        sizes: vec![rel(1), rel(1)],
        rounding: Rounding::Start,
        repeat: false,
        orient: Reorient::KEEP,
        children: vec![
            Node::call("leaf").with_roles([("wall", Material::role("other"))]),
            Node::call("leaf"),
        ],
    }));
    let out = run(&program, [2, 1, 1], 0);
    assert_eq!(out.model.get([0, 0, 0]).unwrap().name, "minecraft:glass");
    assert_eq!(
        out.model.get([1, 0, 0]).unwrap().name,
        "minecraft:stone",
        "the sibling is outside the frame, so it reads the program's own default"
    );
}

/// **An inner frame shadows an outer one**, and the outer one is back in force
/// on the way out.
#[test]
fn an_inner_frame_shadows_the_outer_one() {
    let inner = Node::call("leaf")
        .with_roles([("wall", Material::block(BlockState::simple("purpur_block")))]);
    let program = probe(
        Node::Split(Split {
            axis: Axis::X,
            sizes: vec![rel(1), rel(1)],
            rounding: Rounding::Start,
            repeat: false,
            orient: Reorient::KEEP,
            children: vec![inner, Node::call("leaf")],
        })
        .with_roles([("wall", Material::role("other"))]),
    );
    let out = run(&program, [2, 1, 1], 0);
    assert_eq!(
        out.model.get([0, 0, 0]).unwrap().name,
        "minecraft:purpur_block",
        "the inner frame won"
    );
    assert_eq!(
        out.model.get([1, 0, 0]).unwrap().name,
        "minecraft:glass",
        "and the outer frame is still in force beside it"
    );
}

/// **The bindings of one frame are simultaneous**, evaluated in the enclosing
/// scope: a frame swaps two names rather than chaining them.
#[test]
fn one_frames_bindings_are_simultaneous_not_sequential() {
    let program = Program::new("swap", "start")
        .param("a", 1)
        .param("b", 9)
        .role("wall", BlockState::simple("stone"))
        .rule(
            "start",
            Node::Split(Split {
                axis: Axis::X,
                sizes: vec![
                    Size::Absolute {
                        blocks: Expr::param("a"),
                    },
                    Size::Absolute {
                        blocks: Expr::param("b"),
                    },
                ],
                rounding: Rounding::Truncate,
                repeat: false,
                orient: Reorient::KEEP,
                children: vec![Node::fill("wall"), Node::Void],
            })
            .with_params([("a", Expr::param("b")), ("b", Expr::param("a"))]),
        );
    let out = run(&program, [10, 1, 1], 0);
    // `a` became 9, not 1: had the bindings chained, `b` would have been read
    // after `a` was already 9 and both would be 9, overflowing the box.
    let solid = (0..10)
        .filter(|&x| !out.model.get([x, 0, 0]).unwrap().is_air())
        .count();
    assert_eq!(solid, 9);
}

// ---------------------------------------------------------------------------
// What stops a changing argument from diverging
// ---------------------------------------------------------------------------

/// A recursion whose self-call rebinds `n` to `n + 1`, with a guard on `n`.
fn counting(limit: i64) -> Program {
    Program::new("counting", "step")
        .param("n", 0)
        .param("limit", limit)
        .role("wall", BlockState::simple("stone"))
        .rule_alts(
            "step",
            vec![
                Alternative::new(
                    Node::call("step")
                        .with_params([("n", Expr::param("n").arith(ArithOp::Add, Expr::int(1)))]),
                )
                .when(Cond::cmp(Expr::param("n"), CmpOp::Lt, Expr::param("limit"))),
                Alternative::new(Node::fill("wall")).when(Cond::Otherwise),
            ],
        )
}

/// **A binding gives a recursion a counter, and the counter is what ends it.**
///
/// The box never shrinks here: `n` is the only thing that changes, so this
/// recursion terminates for exactly one reason. It is an index into the
/// *recursion*, not into position.
#[test]
fn a_recursion_can_count_with_a_binding_and_stop_on_it() {
    for limit in [1i64, 5, 40] {
        let out = run(&counting(limit), [1, 1, 1], 0);
        assert_eq!(
            out.stats.rules_applied,
            limit as u64 + 1,
            "limit {limit}: one application per step plus the base case"
        );
    }
}

/// **A binding that never reaches its base case is a `DepthLimit`, not a hang.**
///
/// The budgets on [`Limits`] are what already turn an unguarded recursion into a
/// deterministic, named error, and a changing argument is one more way to write
/// an unguarded recursion — not a new failure mode. Nothing about it is silent
/// and nothing about it is a wall-clock question.
#[test]
fn an_argument_that_never_reaches_its_base_case_hits_the_depth_budget() {
    // `limit` beyond any reachable `n`: the guard `n < limit` never fails.
    let program = counting(i64::MAX);
    let err = expand(
        &program,
        Box3::at_origin([1, 1, 1]),
        &ExpandOptions {
            seed: 0,
            limits: Limits {
                max_depth: 32,
                ..Limits::default()
            },
            orientation: Default::default(),
        },
    )
    .expect_err("an endless recursion must not expand");
    assert_eq!(err, ExpandError::DepthLimit { limit: 32 });
    assert!(format!("{err}").contains("depth limit of 32"));

    // The same program at the default budget is the same error, at the default
    // number: the failure does not depend on how the budget was set.
    let err = expand(
        &program,
        Box3::at_origin([1, 1, 1]),
        &ExpandOptions::seeded(0),
    )
    .expect_err("an endless recursion must not expand");
    assert_eq!(
        err,
        ExpandError::DepthLimit {
            limit: Limits::default().max_depth
        }
    );
}

// ---------------------------------------------------------------------------
// What the construct refuses
// ---------------------------------------------------------------------------

/// A `bind` may only name something the program declares — the reason
/// `set_param` refuses an undeclared parameter, one layer in: a misspelt binding
/// that quietly left the default in place would be green for ever.
#[test]
fn a_binding_may_only_name_something_the_program_declares() {
    let bad_param = probe(Node::call("leaf").with_params([("nope", Expr::int(1))]));
    assert_eq!(
        bad_param.validate(),
        Err(ProgramError::UnknownBinding {
            symbol: "start".into(),
            kind: "parameter",
            name: "nope".into(),
        })
    );
    assert!(format!("{}", bad_param.validate().unwrap_err()).contains("does not declare"));

    let bad_role = probe(Node::call("leaf").with_roles([("nope", Material::role("wall"))]));
    assert_eq!(
        bad_role.validate(),
        Err(ProgramError::UnknownBinding {
            symbol: "start".into(),
            kind: "palette role",
            name: "nope".into(),
        })
    );

    // The VALUE side is checked with the same rules everything else is: a role
    // that is not bound, and a mix with a zero weight, are the errors they
    // already were.
    let bad_value = probe(Node::call("leaf").with_roles([("wall", Material::role("missing"))]));
    assert!(matches!(
        bad_value.validate(),
        Err(ProgramError::UnknownRole { .. })
    ));
    let bad_expr = probe(Node::call("leaf").with_params([("n", Expr::param("missing"))]));
    assert!(matches!(
        bad_expr.validate(),
        Err(ProgramError::UnknownParam { .. })
    ));
}

/// A `bind` that binds nothing is refused where it was written.
#[test]
fn a_bind_that_binds_nothing_is_refused() {
    let empty = probe(Node::Bind {
        params: BTreeMap::new(),
        palette: BTreeMap::new(),
        body: Box::new(Node::call("leaf")),
    });
    assert_eq!(
        empty.validate(),
        Err(ProgramError::EmptyBind {
            symbol: "start".into()
        })
    );
}

/// The authoring form is the JSON, and it round-trips.
#[test]
fn the_json_form_is_the_one_an_author_would_write() {
    let json = serde_json::to_value(library::idioms::arguments()).unwrap();
    let piece = &json["rules"]["piece"][0]["body"];
    // The near pair: an open head, the spine, a glazed head.
    let glazed = &piece["children"][0]["children"][2];
    assert_eq!(glazed["op"], "bind");
    assert_eq!(
        glazed["palette"]["opening"],
        serde_json::json!({ "role": "glazing" })
    );
    assert_eq!(
        glazed["body"],
        serde_json::json!({"op": "call", "symbol": "head"})
    );
    assert!(
        glazed.get("params").is_none(),
        "an unused half of a frame is not serialised"
    );
    // The far pair: the same call, turned, and the frame inside the turn.
    let turned = &piece["children"][2]["children"][2];
    assert_eq!(turned["op"], "reorient");
    assert_eq!(turned["body"]["op"], "bind");
    let back: Program = serde_json::from_value(json).unwrap();
    assert_eq!(back, library::idioms::arguments());
}

/// Composition qualifies a binding's **keys** as well as its values.
///
/// A composed program's parameters and roles carry the include prefix, so a
/// binding that kept the bare name would name something the composition does not
/// have. It is checked here rather than left to `UnknownBinding` because the
/// rewrite is the thing under test.
#[test]
fn an_included_program_keeps_its_bindings() {
    let zone = Program::new("zone", "zone").rule("zone", Node::call("arch/piece"));
    let zone = delvewright_grammar::include(zone, &library::idioms::arguments(), "arch").unwrap();
    zone.validate().unwrap();
    assert!(zone.palette.contains_key("arch/glazing"));
    assert_eq!(
        run(&zone, REGION, 1).model.canonical_bytes(),
        run(&library::idioms::arguments(), REGION, 1)
            .model
            .canonical_bytes(),
        "including a program must not change what it builds"
    );
}

// ---------------------------------------------------------------------------
// The version fence
// ---------------------------------------------------------------------------

/// **`bind` owes the program-document fence an entry the moment that fence
/// exists.**
///
/// `Program::version` and `crates/grammar/src/version.rs` land on PR #417, which
/// is open; rebasing onto it is not available here. So instead of a line in a
/// document asking someone to remember, the obligation is bound to the event
/// that creates it: the day the module lands, this test starts asserting that
/// `bind` is fenced, and reds if it is not.
///
/// It binds to **nothing today, deliberately and visibly** — it prints which
/// state it is in on every run — and to one thing the moment #417 merges.
#[test]
fn bind_is_fenced_the_moment_the_program_version_module_exists() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/version.rs");
    let Ok(source) = std::fs::read_to_string(&path) else {
        println!(
            "binding count 0: {} does not exist yet, so there is no fence for `bind` to be in",
            path.display()
        );
        return;
    };
    println!("binding count 1: {} exists", path.display());
    assert!(
        source.contains("BIND_SINCE") && source.contains("has_bind"),
        "the program-document fence exists but `Node::Bind` is not fenced by it: add \
         `BIND_SINCE` and `has_bind` beside `CONTRACT_SINCE`/`has_contract`, refuse a `bind` in \
         a document that declares an earlier version, and cover both directions with a test"
    );
}
