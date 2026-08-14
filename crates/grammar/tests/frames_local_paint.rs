//! **A paint's frame meeting a scope's frame** — the pairs that exist only where
//! the two halves of the frame and the local axis frame are in one engine.
//!
//! Two constructs answer "this state's directions are named in the scope's own
//! axes". [`Orientation`] is the scope's frame and it has **two** halves: an axis
//! permutation, and a reflection saying which local axes run backwards along the
//! world axis they name. `Paint::Local` is a state read in that frame and
//! resolved into the world's at fill time.
//!
//! Each half was built without the other. A resolver that reads only the
//! permutation answers "the identity moves nothing" for **every purely reflected
//! scope** — the same short circuit to "safe" the `DW0736` judge had before it
//! grew its reflection half, except that a resolver does not merely miss a
//! defect, it WRITES one: the mirror image of the state the author asked for,
//! silently, with every gate green because the gate is reading the resolved
//! state rather than the authored one.
//!
//! So the cases below are the ones neither construct could have had:
//!
//! 1. a local frame inside a **mirrored** body,
//! 2. a local frame inside a scope that **reflects and permutes** at once,
//! 3. a local frame under a **pushed argument frame** (`bind`), where the paint
//!    arrives from an environment rather than from the program's own palette,
//! 4. a local frame inside a **`claim`ed** space, where a second wrapper sits
//!    between the frame and the fill,
//! 5. the **refusal** under a reflection: a yaw and a handedness are stated
//!    against a fixed vertical AND a fixed handedness, so a reflected frame
//!    leaves them no image and `DW0738` says so rather than guessing.
//!
//! Every case is asserted as a **state string**, not as a count: a count would
//! be satisfied by the wrong block.

use std::collections::BTreeMap;

use delvewright_grammar::block::BlockState;
use delvewright_grammar::geom::{Axis, Box3, Mirror};
use delvewright_grammar::ir::{
    Alternative, AxisSpec, Contract, Envelope, Expr, Material, Node, Paint, Program, Reorient,
};
use delvewright_grammar::{ExpandOptions, Expansion, expand, gates};

/// A cube big enough for a fill and small enough to read.
const BOX: Box3 = Box3::at_origin([5, 5, 5]);

/// The bar an author writes when the run spans the scope's own X.
fn local_bar() -> BlockState {
    "minecraft:iron_bars[east=true,north=false,south=false,waterlogged=false,west=false]"
        .parse()
        .unwrap()
}

fn run(program: &Program) -> Expansion {
    expand(program, BOX, &ExpandOptions::seeded(1)).expect("the program expands")
}

/// The distinct `iron_bars` states the model holds, sorted.
fn bars(out: &Expansion) -> Vec<String> {
    let mut v: Vec<String> = out
        .model
        .palette()
        .iter()
        .filter(|s| s.name == "minecraft:iron_bars")
        .map(ToString::to_string)
        .collect();
    v.sort();
    v
}

/// A program that fills the whole box with the local-frame bar, under `frame`.
fn framed(frame: Reorient) -> Program {
    Program::new("framed", "start")
        .role_local("bar", local_bar())
        .rule_alts(
            "start",
            vec![Alternative::new(Node::Reorient {
                orient: frame,
                body: Box::new(Node::fill("bar")),
            })],
        )
}

/// Every gate green, and the ones this file cares about named so a rename is a
/// red rather than a silent skip.
fn all_green(out: &Expansion) {
    let report = gates::judge(out, gates::Options::default());
    let ids: Vec<&str> = report.gates.iter().map(|g| g.id).collect();
    for wanted in ["oriented-fills", "shape-complete", "states-complete"] {
        assert!(ids.contains(&wanted), "{ids:?}");
    }
    assert!(report.is_pass(), "{:#?}", report.gates);
}

// ---------------------------------------------------------------------------
// 1. A local frame inside a mirrored body
// ---------------------------------------------------------------------------

/// **The case the short circuit gets wrong.** A pure reflection on the local X
/// has the IDENTITY axis permutation, so a resolver reading only the permutation
/// writes the state through untouched — and the piece is built the other way
/// round. The bar's run ends at the scope's `east`; in a body whose local X runs
/// backwards, that end is the world's `west`.
#[test]
fn a_local_frame_inside_a_mirrored_body_resolves_through_the_reflection() {
    let out = run(&framed(Reorient::KEEP.flip(Axis::X)));
    assert_eq!(
        bars(&out),
        ["minecraft:iron_bars[east=false,north=false,south=false,waterlogged=false,west=true]"],
        "a reflected local X sends the scope's east to the world's west"
    );
    all_green(&out);

    // The unreflected frame is the control: same program, same role, and the
    // literal is written as authored. Without this the assertion above would
    // pass for a resolver that rewrote everything.
    let flat = run(&framed(Reorient::KEEP));
    assert_eq!(
        bars(&flat),
        ["minecraft:iron_bars[east=true,north=false,south=false,waterlogged=false,west=false]"]
    );
    assert_ne!(
        bars(&out),
        bars(&flat),
        "the reflection must move the bytes"
    );

    // And reflecting an axis the state does not name changes nothing, so this
    // is not "any mirror rewrites everything".
    let other = run(&framed(Reorient::KEEP.flip(Axis::Z)));
    assert_eq!(bars(&other), bars(&flat));
}

/// Reflecting twice is the identity frame, so a local paint nested inside two
/// reflections of the same axis is written exactly as authored — the property
/// that lets one rule stand at both sites of a mirror pair.
#[test]
fn two_reflections_cancel_for_a_local_paint_as_they_do_for_geometry() {
    let program = Program::new("twice", "start")
        .role_local("bar", local_bar())
        .rule_alts(
            "start",
            vec![Alternative::new(Node::Reorient {
                orient: Reorient::KEEP.flip(Axis::X),
                body: Box::new(Node::Reorient {
                    orient: Reorient::KEEP.flip(Axis::X),
                    body: Box::new(Node::fill("bar")),
                }),
            })],
        );
    let out = run(&program);
    assert_eq!(bars(&out), bars(&run(&framed(Reorient::KEEP))));
    all_green(&out);
}

// ---------------------------------------------------------------------------
// 2. A local frame inside a scope that reflects AND permutes
// ---------------------------------------------------------------------------

/// **The composite is a different map from either half.** Swapping X and Z is
/// itself a reflection of the horizontal plane; adding a Z reflection turns the
/// composite into a quarter turn, and the bar's run lands on the opposite side
/// from where the bare swap puts it. A resolver that dropped either half would
/// produce one of the two wrong answers below.
#[test]
fn a_local_frame_under_a_composite_frame_composes_both_halves() {
    let swap = Reorient::KEEP.x(AxisSpec::LocalZ).z(AxisSpec::LocalX);

    let bare = run(&framed(swap));
    assert_eq!(
        bars(&bare),
        ["minecraft:iron_bars[east=false,north=false,south=true,waterlogged=false,west=false]"],
        "local X is world Z, so the scope's east is the world's south"
    );

    let composite = run(&framed(swap.flip(Axis::X)));
    assert_eq!(
        bars(&composite),
        ["minecraft:iron_bars[east=false,north=true,south=false,waterlogged=false,west=false]"],
        "…and reflecting that axis sends it to the world's north instead"
    );

    assert_ne!(bars(&bare), bars(&composite), "the sign is load-bearing");
    assert_ne!(
        bars(&composite),
        bars(&run(&framed(Reorient::KEEP.flip(Axis::X)))),
        "and so is the permutation"
    );
    all_green(&bare);
    all_green(&composite);
}

// ---------------------------------------------------------------------------
// 3. A local frame under a pushed argument frame
// ---------------------------------------------------------------------------

/// **A pushed paint is resolved in the SCOPE's frame, not the caller's.** A
/// `bind` pushes a palette override down a call; the frame a local state is read
/// in belongs to the box being filled, so pushing the same paint into two
/// differently framed scopes writes two different states from one binding. That
/// is the whole point of the frame, and a `bind` is where it could most easily
/// have been lost — the paint arrives from an environment rather than from the
/// program's own palette, and it is a different code path.
#[test]
fn a_pushed_paint_is_read_in_the_frame_of_the_scope_it_lands_in() {
    let pushed = Material::Inline(Paint::local_block(local_bar()));
    let program = Program::new("pushed", "start")
        // A placeholder binding the push replaces: `fill` needs a bound role,
        // and binding it to something visibly different proves the push landed.
        .role("bar", BlockState::simple("minecraft:air"))
        .rule_alts(
            "start",
            vec![Alternative::new(
                Node::Reorient {
                    orient: Reorient::KEEP.flip(Axis::X),
                    body: Box::new(Node::call("inner")),
                }
                .with_roles([("bar", pushed.clone())]),
            )],
        )
        .rule("inner", Node::fill("bar"));
    let out = run(&program);
    assert_eq!(
        bars(&out),
        ["minecraft:iron_bars[east=false,north=false,south=false,waterlogged=false,west=true]"],
        "the push crossed a reorient and a call, and was read in the inner frame"
    );
    all_green(&out);

    // The same push into an unreflected scope: one binding, two states.
    let flat = Program::new("pushed-flat", "start")
        .role("bar", BlockState::simple("minecraft:air"))
        .rule_alts(
            "start",
            vec![Alternative::new(
                Node::call("inner").with_roles([("bar", pushed)]),
            )],
        )
        .rule("inner", Node::fill("bar"));
    assert_eq!(
        bars(&run(&flat)),
        ["minecraft:iron_bars[east=true,north=false,south=false,waterlogged=false,west=false]"]
    );
}

/// A pushed paint is fenced like any other: the frame belongs to the STATE, so
/// an inline `local` on a `bind` is refused by a document that declares a
/// version predating the surface. A fence checked only on the palette would be
/// one a document walks around by moving the state onto the push.
#[test]
fn a_pushed_local_paint_is_fenced_where_it_is_written() {
    let program = Program::new("pushed", "start")
        .role("bar", BlockState::simple("minecraft:air"))
        .rule_alts(
            "start",
            vec![Alternative::new(Node::fill("bar").with_roles([(
                "bar",
                Material::Inline(Paint::local_block(local_bar())),
            )]))],
        )
        .at_version("1.3.0");
    let err = program.validate().expect_err("1.3.0 predates the frame");
    let text = err.to_string();
    assert!(text.contains("1.4.0"), "{text}");
    assert!(text.contains("local"), "{text}");
}

// ---------------------------------------------------------------------------
// 4. A local frame inside a claimed space
// ---------------------------------------------------------------------------

/// **A `claim` between the frame and the fill changes neither.** The claim is a
/// wrapper that names the scope's box for the spatial contract and writes no
/// blocks; the frame it inherits is the one the fill is resolved in. Asserted
/// both ways round — the resolved state is the mirrored one, AND the claim still
/// records the box — because a wrapper that quietly reset the frame would look
/// exactly like a wrapper that worked.
#[test]
fn a_local_frame_inside_a_claimed_space_keeps_the_frame_and_the_claim() {
    let program = Program::new("claimed", "start")
        .role_local("bar", local_bar())
        .contract(Contract::new("cell").space("cell", Envelope::Open))
        .rule_alts(
            "start",
            vec![Alternative::new(Node::Reorient {
                orient: Reorient::KEEP.flip(Axis::X),
                body: Box::new(Node::Claim {
                    region: "cell".to_string(),
                    body: Box::new(Node::fill("bar")),
                }),
            })],
        );
    let out = run(&program);
    assert_eq!(
        bars(&out),
        ["minecraft:iron_bars[east=false,north=false,south=false,waterlogged=false,west=true]"],
        "the claim is a wrapper, not a frame"
    );
    let contract = delvewright_grammar::export::contract_metadata(&out)
        .expect("the program declares a contract");
    assert!(
        contract.spaces.contains_key("cell"),
        "the claim still recorded its box: {contract:?}"
    );
    assert_eq!(out.oriented.resolved, 1, "the frame's binding count");
}

// ---------------------------------------------------------------------------
// 5. The refusal under a reflection
// ---------------------------------------------------------------------------

/// **A yaw has no image under a reflected frame, and the build says so.** A
/// 16-step `rotation` is stated against a fixed vertical and a fixed handedness,
/// so the frames that determine it are the pure turns about the vertical. A
/// reflection is not one, and the answer is `DW0738` naming the state, the
/// property and the frame — never a plausible skull.
///
/// The control is the same role in an unreflected scope: resolved, not refused.
/// Without it this would pass for an engine that refused every local frame.
#[test]
fn a_yaw_in_a_mirrored_body_is_refused_rather_than_reflected() {
    let skull: BlockState = "minecraft:skeleton_skull[powered=false,rotation=8]"
        .parse()
        .unwrap();
    let program = |frame: Reorient| {
        Program::new("corpse", "start")
            .role_local("corpse", skull.clone())
            .rule_alts(
                "start",
                vec![Alternative::new(Node::Reorient {
                    orient: frame,
                    body: Box::new(Node::fill("corpse")),
                })],
            )
    };

    let err = expand(
        &program(Reorient::KEEP.flip(Axis::Z)),
        BOX,
        &ExpandOptions::seeded(1),
    )
    .expect_err("a reflected frame determines no yaw");
    let text = err.to_string();
    assert!(text.contains("DW0738"), "{text}");
    assert!(text.contains("rotation=8"), "{text}");
    assert!(
        text.contains("-Z"),
        "the frame is printed with its sign: {text}"
    );

    // The control, and the reason the refusal is about the FRAME rather than
    // about the construct.
    let out = expand(&program(Reorient::KEEP), BOX, &ExpandOptions::seeded(1))
        .expect("an unreflected frame resolves it");
    assert_eq!(out.oriented.resolved, 1);

    // A handedness refuses on the same rule and for the same reason: a mirror
    // is exactly what swaps it, and a reflected frame is outside the vocabulary
    // that says by how much.
    let door: BlockState =
        "minecraft:oak_door[facing=north,half=lower,hinge=left,open=false,powered=false]"
            .parse()
            .unwrap();
    let hinged = Program::new("hinge", "start")
        .role_local("door", door)
        .rule_alts(
            "start",
            vec![Alternative::new(Node::Reorient {
                orient: Reorient::KEEP.mirror(Mirror::of(Axis::X)),
                body: Box::new(Node::fill("door")),
            })],
        );
    let text = expand(&hinged, BOX, &ExpandOptions::seeded(1))
        .expect_err("a reflected frame determines no handedness")
        .to_string();
    assert!(text.contains("DW0738"), "{text}");
    assert!(text.contains("hinge=left"), "{text}");
}

/// **The judge and the resolver never disagree about a reflected frame.** Over
/// every frame the grammar can build — six permutations by eight reflections —
/// a local paint either resolves or is refused, and the refusals are exactly the
/// states the `DW0736` judge calls wrong when the same literal is written in the
/// world frame. Binding counts are asserted on both outcomes: a sweep that only
/// ever resolved, or only ever refused, would discriminate nothing.
#[test]
fn over_every_frame_a_local_paint_resolves_or_refuses_and_never_guesses() {
    let cases: [(&str, BlockState); 2] = [
        ("bar", local_bar()),
        (
            "corpse",
            "minecraft:skeleton_skull[powered=false,rotation=8]"
                .parse()
                .unwrap(),
        ),
    ];
    let specs = [
        (AxisSpec::LocalX, AxisSpec::LocalY, AxisSpec::LocalZ),
        (AxisSpec::LocalZ, AxisSpec::LocalY, AxisSpec::LocalX),
        (AxisSpec::LocalX, AxisSpec::LocalZ, AxisSpec::LocalY),
        (AxisSpec::LocalY, AxisSpec::LocalX, AxisSpec::LocalZ),
        (AxisSpec::LocalY, AxisSpec::LocalZ, AxisSpec::LocalX),
        (AxisSpec::LocalZ, AxisSpec::LocalX, AxisSpec::LocalY),
    ];

    let mut examined = 0usize;
    let mut resolved = 0usize;
    let mut refused = 0usize;
    for (role, state) in &cases {
        for (x, y, z) in specs {
            for bits in 0..8u8 {
                let mut mirror = Mirror::NONE;
                for (i, axis) in Axis::ALL.into_iter().enumerate() {
                    if bits & (1 << i) != 0 {
                        mirror = mirror.and(axis);
                    }
                }
                let frame = Reorient::KEEP.x(x).y(y).z(z).mirror(mirror);
                let program = Program::new("sweep", "start")
                    .role_local(role, state.clone())
                    .rule_alts(
                        "start",
                        vec![Alternative::new(Node::Reorient {
                            orient: frame,
                            body: Box::new(Node::fill(role)),
                        })],
                    );
                examined += 1;
                match expand(&program, BOX, &ExpandOptions::seeded(1)) {
                    Ok(out) => {
                        resolved += 1;
                        // Whatever it wrote, the pin accepts it and the
                        // `DW0736` gate is silent about it: a resolver that
                        // invented a state would red its own build.
                        let report = gates::judge(&out, gates::Options::default());
                        assert!(report.is_pass(), "{frame:?}: {:#?}", report.gates);
                    }
                    Err(e) => {
                        refused += 1;
                        let text = e.to_string();
                        assert!(text.contains("DW0738"), "{frame:?}: {text}");
                    }
                }
            }
        }
    }
    assert_eq!(examined, 2 * 6 * 8, "binding count: states x frames");
    assert!(
        resolved > 0 && refused > 0,
        "binding count {resolved} resolved / {refused} refused of {examined}: a sweep with \
         only one outcome discriminates nothing"
    );
    println!(
        "local paint over every frame  bound {examined}  ({resolved} resolved, {refused} refused)"
    );
}

// ---------------------------------------------------------------------------
// Determinism, with a control that is not the seed
// ---------------------------------------------------------------------------

/// **Byte-stable, and the comparison discriminates.** The frame is read off the
/// scope and never off a draw, so a program with no weighted alternative is the
/// same model at any seed — which makes the seed an INVALID control here, and
/// pinning two seeds equal would be a comparison that proves nothing. The
/// control is the FRAME: reflecting one axis must move the bytes.
#[test]
fn a_local_paint_is_byte_stable_and_the_frame_is_the_control() {
    let bytes = |frame: Reorient| {
        expand(&framed(frame), BOX, &ExpandOptions::seeded(1))
            .unwrap()
            .model
            .canonical_bytes()
    };
    let seeded = |seed: u64| {
        expand(
            &framed(Reorient::KEEP.flip(Axis::X)),
            BOX,
            &ExpandOptions::seeded(seed),
        )
        .unwrap()
        .model
        .canonical_bytes()
    };
    assert_eq!(seeded(1), seeded(1), "same seed, same bytes");
    assert_eq!(
        seeded(1),
        seeded(7),
        "no weighted alternative, so the seed is not a control here"
    );
    assert_ne!(
        bytes(Reorient::KEEP),
        bytes(Reorient::KEEP.flip(Axis::X)),
        "the frame IS the control, and it moves the bytes"
    );
}

/// A `bind` pushing params alongside a local paint keeps both, so the two
/// wrappers do not clobber one another — the interaction a merge is the first
/// place to have.
#[test]
fn a_bind_carries_a_param_and_a_local_paint_at_once() {
    let mut params = BTreeMap::new();
    params.insert("thickness".to_string(), Expr::Int { value: 2 });
    let program = Program::new("both", "start")
        .param("thickness", 1)
        .role("bar", BlockState::simple("minecraft:air"))
        .rule_alts(
            "start",
            vec![Alternative::new(Node::Bind {
                params,
                palette: [(
                    "bar".to_string(),
                    Material::Inline(Paint::local_block(local_bar())),
                )]
                .into_iter()
                .collect(),
                body: Box::new(Node::Reorient {
                    orient: Reorient::KEEP.flip(Axis::X),
                    body: Box::new(Node::fill("bar")),
                }),
            })],
        );
    let out = run(&program);
    assert_eq!(
        bars(&out),
        ["minecraft:iron_bars[east=false,north=false,south=false,waterlogged=false,west=true]"]
    );
    all_green(&out);
}
