//! **The blockstate family across the frame constructs** — the interactions
//! that exist only where the two lines of work meet.
//!
//! `DW0735`/`DW0736` were written against a scope whose frame was a
//! *permutation*: local axes renamed onto world axes. The frame has since grown
//! a second half — a **reflection**, which says which way along that world axis
//! a local axis runs — plus two constructs that push a scope without touching
//! either half, `bind` and `claim`. Neither line of work could test the pairs,
//! because on each branch alone only one of the two existed.
//!
//! The pair that matters most is the first one below, and it is not a matter of
//! taste. A predicate that reads only the permutation answers `None` for every
//! purely reflected frame — the identity permutation short-circuits — and `None`
//! is the answer that means "safe". So a mirrored arm of a transept could fill
//! `facing=north` where the world wants south, and every gate in the toolchain
//! would stay green over it. `tests/shape_orient.rs` covers the same two
//! diagnostics under a turned box; this file covers them under a *reflected*
//! one, under a pushed binding frame, and under a claim.

use delvewright_grammar::block::BlockState;
use delvewright_grammar::export::{ExportError, export_zone};
use delvewright_grammar::gates;
use delvewright_grammar::geom::{Axis, Mirror};
use delvewright_grammar::ir::{
    Alternative, Cond, Contract, EXTERIOR, EdgeClass, Envelope, Material, Node, Program, Reorient,
    Rounding, Size, Split,
};
use delvewright_grammar::{Box3, ExpandOptions, Expansion, expand};

/// A box longer in Z than in X, so `Reorient::KEEP` really does keep: the
/// permutation half of every frame below is the identity, and the reflection
/// half is the only thing under test.
const PIECE: Box3 = Box3::at_origin([5, 4, 9]);

fn run(program: &Program) -> Expansion {
    expand(program, PIECE, &ExpandOptions::seeded(1)).expect("the program expands")
}

fn gate<'r>(report: &'r gates::Report, id: &str) -> &'r gates::Gate {
    report
        .gates
        .iter()
        .find(|g| g.id == id)
        .unwrap_or_else(|| panic!("no `{id}` gate in {:?}", report.gates))
}

fn judge(out: &Expansion) -> gates::Report {
    gates::judge(out, gates::Options::default())
}

fn fill_state(state: &str) -> Node {
    Node::Fill {
        material: Material::block(state.parse::<BlockState>().unwrap()),
    }
}

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

fn reflected(axis: Axis, body: Node) -> Node {
    Node::Reorient {
        orient: Reorient::KEEP.flip(axis),
        body: Box::new(body),
    }
}

/// The scope's own frame is unreflected identity, and the body is expanded in
/// it — the control every red below is measured against.
fn plain(_axis: Axis, body: Node) -> Node {
    Node::Reorient {
        orient: Reorient::KEEP,
        body: Box::new(body),
    }
}

// ---------------------------------------------------------------------------
// 1. An orientation-sensitive fill inside a MIRRORED body
// ---------------------------------------------------------------------------

/// One stair, filled into a scope whose frame is reflected on `wrapped`.
fn stair_under(wrapped: fn(Axis, Node) -> Node, axis: Axis) -> Program {
    Program::new("mirrored_stair", "piece").rule(
        "piece",
        wrapped(axis, fill_state("oak_stairs[facing=north,half=bottom]")),
    )
}

/// **The hole.** A reflection is not a permutation — no rotation reproduces one
/// — so a frame that only *reflects* has the identity permutation, and a
/// predicate reading the permutation alone says "nothing moved".
///
/// It moved. Local `Z` runs the other way, so the author's local north is world
/// south, and the stair the rule filled faces backwards in the world. The gate
/// must red, and the finding must name the reflection.
#[test]
fn an_oriented_fill_inside_a_mirrored_body_is_a_finding() {
    let out = run(&stair_under(reflected, Axis::Z));
    assert!(
        out.oriented.carrying > 0,
        "the fill must be examined, not skipped: {:?}",
        out.oriented
    );

    let report = judge(&out);
    let g = gate(&report, "oriented-fills");
    assert!(
        !g.passed(),
        "a mirrored frame lands the literal facing backwards: {}",
        g.detail
    );
    assert!(g.bound > 0, "the gate examined nothing");
    assert!(g.detail.contains("DW0736"), "{}", g.detail);
    assert_eq!(out.oriented.unguarded.len(), 1, "{:?}", out.oriented);
    assert_eq!(out.oriented.unguarded[0].property, "facing=north");
}

/// **And it is not a blanket refusal of mirrored scopes.** Reflecting an axis
/// the state does not name changes nothing about it: the mirror image of a
/// north-facing stair across the east-west axis still faces north.
///
/// This is the assertion that keeps the check above from being satisfied by a
/// predicate that simply reds every reflection — which would be green on this
/// test's sibling and useless in a building with two arms.
#[test]
fn a_reflection_of_an_axis_the_state_does_not_name_is_no_finding() {
    let out = run(&stair_under(reflected, Axis::X));
    assert!(out.oriented.carrying > 0, "{:?}", out.oriented);
    let report = judge(&out);
    let g = gate(&report, "oriented-fills");
    assert!(g.passed(), "{}", g.detail);
    assert!(g.bound > 0);
    assert!(out.oriented.unguarded.is_empty(), "{:?}", out.oriented);
}

/// The unreflected control: the same fill in the same box under `KEEP` is
/// green, so the red above is caused by the reflection and by nothing else.
#[test]
fn the_same_fill_in_an_unreflected_frame_is_green() {
    let out = run(&stair_under(plain, Axis::Z));
    let report = judge(&out);
    assert!(gate(&report, "oriented-fills").passed());
    assert!(out.oriented.unguarded.is_empty());
}

/// **Green, and not by silencing.** The guard names the frame *exactly*,
/// reflection included, so an author who wants a stair in both arms of a mirror
/// pair writes one alternative per arm — and the reflected alternative carries
/// the reflected facing, which is what actually lands right in the world.
///
/// The model is inspected rather than only the verdict: a guard that silenced
/// the gate while still writing `facing=north` would pass a verdict-only test.
#[test]
fn a_frame_guard_licenses_the_mirrored_arm_and_writes_the_reflected_facing() {
    let program = Program::new("guarded_stair", "piece")
        .rule("piece", reflected(Axis::Z, Node::call("stair")))
        .rule_alts(
            "stair",
            vec![
                Alternative::new(fill_state("oak_stairs[facing=north,half=bottom]"))
                    .when(Cond::orientation(Axis::X, Axis::Y, Axis::Z)),
                Alternative::new(fill_state("oak_stairs[facing=south,half=bottom]"))
                    .when(Cond::frame(Axis::X, Axis::Y, Axis::Z, Mirror::of(Axis::Z))),
            ],
        );
    let out = run(&program);
    assert!(out.oriented.carrying > 0, "{:?}", out.oriented);
    let report = judge(&out);
    assert!(gate(&report, "oriented-fills").passed());
    assert!(
        out.model
            .palette()
            .iter()
            .any(|s| s.to_string().contains("facing=south")),
        "the reflected arm must carry the reflected facing: {:?}",
        out.model
            .palette()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    );
}

/// **The unreflected guard does NOT license the reflected arm.** This is the
/// half a permissive guard would fail: if `Cond::Orientation` ignored the
/// reflection, the first alternative would be selected in both arms and the
/// north-facing stair would ship in the mirror image.
#[test]
fn an_unreflected_guard_does_not_license_a_reflected_scope() {
    let program = Program::new("half_guarded_stair", "piece")
        .rule("piece", reflected(Axis::Z, Node::call("stair")))
        .rule_alts(
            "stair",
            vec![
                Alternative::new(fill_state("oak_stairs[facing=north,half=bottom]"))
                    .when(Cond::orientation(Axis::X, Axis::Y, Axis::Z)),
                // The fallback exists so the expansion has an alternative to
                // take at all; it writes the same literal, which is the defect.
                Alternative::new(fill_state("oak_stairs[facing=north,half=bottom]")),
            ],
        );
    let out = run(&program);
    let report = judge(&out);
    let g = gate(&report, "oriented-fills");
    assert!(
        !g.passed(),
        "the guard named an unreflected frame and cannot license this one: {}",
        g.detail
    );
    assert_eq!(out.oriented.unguarded.len(), 1, "{:?}", out.oriented);
}

// ---------------------------------------------------------------------------
// 2. An export refusal raised inside a reframed subtree — is it findable?
// ---------------------------------------------------------------------------

/// **A refusal has to name a place the author can go and look at.**
///
/// The finding carries the rule symbol and the scope's frame. The frame is
/// where a reflection can hide: printed as a permutation alone, a reflected
/// identity frame reads `x->X,y->Y,z->Z` — which is the identity, so the
/// message would point an author at a frame nothing turned, and the reflection
/// that actually turned their stair would appear nowhere in it.
///
/// So the label carries the sign, and this test is what holds it there: the
/// mirrored frame's label must differ from the unreflected one.
#[test]
fn a_refusal_inside_a_reflected_subtree_names_the_reflection() {
    let program = stair_under(reflected, Axis::Z);
    let err = export_zone(&program, PIECE, &ExpandOptions::seeded(1), "piece").unwrap_err();
    assert!(
        matches!(err, ExportError::UnguardedOrientedFills { .. }),
        "{err}"
    );
    let message = err.to_string();
    assert!(message.contains("DW0736"), "{message}");
    assert!(message.contains("piece"), "the rule is named: {message}");
    assert!(
        message.contains("z->-Z"),
        "the reflected axis must be marked, or the frame reads as identity: {message}"
    );

    // The unreflected frame's label is a different string — which is what makes
    // the marking above information rather than decoration.
    let turned = expand(
        &Program::new("turned_stair", "piece").rule(
            "piece",
            Node::Reorient {
                orient: Reorient::KEEP.z(delvewright_grammar::ir::AxisSpec::LocalX),
                body: Box::new(fill_state("oak_stairs[facing=north,half=bottom]")),
            },
        ),
        PIECE,
        &ExpandOptions::seeded(1),
    )
    .unwrap();
    assert_eq!(turned.oriented.unguarded.len(), 1, "{:?}", turned.oriented);
    let turned_frame = &turned.oriented.unguarded[0].orientation;
    assert!(
        !turned_frame.contains("->-"),
        "a turned frame reflects nothing, so no axis carries the sign: {turned_frame}"
    );

    let mirrored = run(&program);
    assert_eq!(mirrored.oriented.unguarded.len(), 1);
    let mirrored_frame = &mirrored.oriented.unguarded[0].orientation;
    assert!(mirrored_frame.contains("->-"), "{mirrored_frame}");
    assert_ne!(
        turned_frame, mirrored_frame,
        "a turned frame and a reflected one must not print the same label"
    );
    // The reflected frame's permutation half IS the identity — which is exactly
    // why printing the permutation alone would have lost the whole finding.
    assert_eq!(mirrored_frame.replace("->-", "->"), "x->X,y->Y,z->Z");
}

// ---------------------------------------------------------------------------
// 3. Shape completeness under a pushed BINDING frame
// ---------------------------------------------------------------------------

/// A program whose bars come from a role, so the state a `bind` supplies is the
/// state the fill writes.
fn barred(role_state: &str, rebind: Option<&str>) -> Program {
    let body = Node::fill("bar");
    let body = match rebind {
        Some(state) => {
            body.with_roles([("bar", Material::block(state.parse::<BlockState>().unwrap()))])
        }
        None => body,
    };
    Program::new("barred", "piece")
        .role("bar", role_state.parse::<BlockState>().unwrap())
        .rule("piece", body)
}

/// **`DW0735` reaches through a `bind`.** A `bind` resolves a role to a state
/// the program's own palette never held, so a shape gate that consulted the
/// *program* palette instead of the scope's would be looking at the wrong
/// state — green while the model holds isolated posts.
///
/// Red half: the program's role is complete, the binding overrides it with a
/// bare `iron_bars`, and the gate must red on what was actually filled.
#[test]
fn a_bind_that_rebinds_a_role_to_an_incomplete_state_is_a_finding() {
    let complete = "iron_bars[east=true,north=false,south=false,west=true]";
    let out = run(&barred(complete, Some("iron_bars")));
    let report = judge(&out);
    let g = gate(&report, "shape-complete");
    assert!(!g.passed(), "{}", g.detail);
    assert!(g.bound > 0, "the gate examined no placed state");
    assert!(g.detail.contains("DW0735"), "{}", g.detail);
    assert!(
        g.detail.contains("east, north, south, west"),
        "{}",
        g.detail
    );

    let err = export_zone(
        &barred(complete, Some("iron_bars")),
        PIECE,
        &ExpandOptions::seeded(1),
        "piece",
    )
    .unwrap_err();
    assert!(matches!(err, ExportError::ShapeOmissions { .. }), "{err}");
}

/// The other direction: the program's role is the bare one and the binding
/// repairs it. Green, and the complete state is what reached the model — so the
/// gate is reading the bound paint and not merely agreeing with the palette.
#[test]
fn a_bind_that_rebinds_a_role_to_a_complete_state_is_green() {
    let complete = "iron_bars[east=true,north=false,south=false,west=true]";
    let out = run(&barred("iron_bars", Some(complete)));
    let report = judge(&out);
    let g = gate(&report, "shape-complete");
    assert!(g.passed(), "{}", g.detail);
    assert!(g.bound > 0);
    assert!(
        out.model
            .palette()
            .iter()
            .any(|s| s.to_string() == format!("minecraft:{complete}")),
        "{:?}",
        out.model
            .palette()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    );
}

/// **A frame cannot hide a shape defect, and must not be able to.** Shape
/// completeness is a property of the *state*, not of where it was written, so
/// the bare bars red identically under a reflection, a turn and neither.
///
/// Stated as a test because the two gates read the same fills and it would be
/// easy for the shape half to acquire the orientation half's identity
/// short-circuit by proximity.
#[test]
fn shape_completeness_is_unmoved_by_the_frame() {
    let bare = || Node::fill("bar");
    let program = |body: Node| {
        Program::new("framed_bars", "piece")
            .role("bar", "iron_bars".parse::<BlockState>().unwrap())
            .rule("piece", body)
    };
    let mut bound = 0usize;
    for body in [
        bare(),
        plain(Axis::Z, bare()),
        reflected(Axis::Z, bare()),
        reflected(Axis::X, bare()),
        Node::Reorient {
            orient: Reorient::KEEP.z(delvewright_grammar::ir::AxisSpec::LocalX),
            body: Box::new(bare()),
        },
    ] {
        let out = run(&program(body));
        let report = judge(&out);
        let g = gate(&report, "shape-complete");
        assert!(
            !g.passed(),
            "a frame must not excuse a bare state: {}",
            g.detail
        );
        assert!(g.bound > 0);
        bound += g.bound;
    }
    assert!(
        bound > 0,
        "every frame in the sweep bound at least one state"
    );
}

// ---------------------------------------------------------------------------
// 4. A `claim` inside a reoriented / reflected scope
// ---------------------------------------------------------------------------

/// Two pieces along local `X`; the first is claimed as `head`. Under a
/// reflected frame the split lays its first piece from the *world*-high end, so
/// `head` must follow it there.
fn claimed_pair(orient: Reorient) -> Program {
    Program::new("claimed_pair", "piece")
        .role("mass", BlockState::simple("stone_bricks"))
        .contract(
            Contract::new("head")
                .space("head", Envelope::Open)
                .space("tail", Envelope::Open)
                .edge(
                    EXTERIOR,
                    "head",
                    EdgeClass::Walk {
                        rise: 0,
                        via: None,
                        way: None,
                    },
                ),
        )
        .rule(
            "piece",
            Node::Reorient {
                orient,
                body: Box::new(Node::Split(Split {
                    axis: Axis::X,
                    sizes: vec![Size::abs(2), Size::rel(1)],
                    rounding: Rounding::Start,
                    repeat: false,
                    orient: Reorient::KEEP,
                    children: vec![
                        Node::Claim {
                            region: "head".to_string(),
                            body: Box::new(Node::fill("mass")),
                        },
                        Node::Claim {
                            region: "tail".to_string(),
                            body: Box::new(Node::fill("mass")),
                        },
                    ],
                })),
            },
        )
}

/// **A claim is resolved in the frame the scope is actually in.** It records
/// the scope's box, and a reflection is what decides which box the split's
/// first piece got — so the claimed region follows the reflection with the
/// blocks, rather than staying where an unreflected reading would have put it.
///
/// The mirror image is asserted exactly: 2 cells wide at world `X`-min plain,
/// 2 cells wide at world `X`-max reflected, same volume either way.
#[test]
fn a_claim_follows_the_reflection_the_split_followed() {
    let plain = run(&claimed_pair(Reorient::KEEP));
    let flipped = run(&claimed_pair(Reorient::KEEP.flip(Axis::X)));

    let head_of = |out: &Expansion| {
        let c = out.contract.as_ref().expect("the program declares one");
        let region = &c.spaces["head"].region;
        assert_eq!(region.boxes.len(), 1, "{:?}", region.boxes);
        (region.boxes[0].origin, region.boxes[0].size, region.cells())
    };

    let (plain_origin, plain_size, plain_cells) = head_of(&plain);
    let (flip_origin, flip_size, flip_cells) = head_of(&flipped);

    assert_eq!(
        plain_origin[0], 0,
        "unreflected: the head is at world X-min"
    );
    assert_eq!(plain_size[0], 2);
    assert_eq!(
        flip_origin[0],
        PIECE.size[0] as i32 - 2,
        "reflected: the head is at world X-max"
    );
    assert_eq!(flip_size[0], 2);
    assert_eq!(
        plain_cells, flip_cells,
        "the same rule claims the same number of cells in either frame"
    );
    assert!(plain_cells > 0, "the claim bound to nothing");

    // And the boxes really are different — a claim that ignored the frame would
    // make this assertion pass by accident.
    assert_ne!(plain_origin, flip_origin);
}

/// **A claim changes no block, in any frame.** `claim` writes nothing, so the
/// model under a claimed program is byte-identical to the same program with the
/// claims removed — and that has to hold in a reflected frame too, which is the
/// pairing neither line of work could test.
#[test]
fn a_claim_moves_no_block_in_a_reflected_frame() {
    let unclaimed = |orient: Reorient| {
        Program::new("plain_pair", "piece")
            .role("mass", BlockState::simple("stone_bricks"))
            .rule(
                "piece",
                Node::Reorient {
                    orient,
                    body: Box::new(split(
                        Axis::X,
                        vec![Size::abs(2), Size::rel(1)],
                        vec![Node::fill("mass"), Node::fill("mass")],
                    )),
                },
            )
    };
    let mut compared = 0;
    for orient in [Reorient::KEEP, Reorient::KEEP.flip(Axis::X)] {
        assert_eq!(
            run(&claimed_pair(orient)).model.canonical_bytes(),
            run(&unclaimed(orient)).model.canonical_bytes(),
            "a claim moved a block"
        );
        compared += 1;
    }
    assert_eq!(compared, 2, "both frames were compared");
}

/// **A `claim` does not disturb the guard's licence.** `claim` moves neither
/// half of the frame, so a fill under a guard that passed upstream of the claim
/// is still licensed — and a fill under no guard is still a finding. Both
/// directions, because a construct that silently *cleared* the pin would look
/// correct from the red side alone.
#[test]
fn a_claim_neither_grants_nor_revokes_the_guards_licence() {
    let program = |guarded: bool| {
        let fill = fill_state("oak_stairs[facing=south,half=bottom]");
        let body = Node::Claim {
            region: "head".to_string(),
            body: Box::new(fill),
        };
        let alt = if guarded {
            Alternative::new(body).when(Cond::frame(Axis::X, Axis::Y, Axis::Z, Mirror::of(Axis::Z)))
        } else {
            Alternative::new(body)
        };
        Program::new("claim_under_guard", "piece")
            .contract(Contract::new("head").space("head", Envelope::Open).edge(
                EXTERIOR,
                "head",
                EdgeClass::Walk {
                    rise: 0,
                    via: None,
                    way: None,
                },
            ))
            .rule("piece", reflected(Axis::Z, Node::call("inner")))
            .rule_alts("inner", vec![alt])
    };

    let guarded = run(&program(true));
    assert!(guarded.oriented.carrying > 0, "{:?}", guarded.oriented);
    assert!(
        guarded.oriented.unguarded.is_empty(),
        "the claim revoked a licence it has no business touching: {:?}",
        guarded.oriented
    );
    assert!(
        guarded.contract.as_ref().unwrap().spaces["head"]
            .region
            .cells()
            > 0
    );

    let unguarded = run(&program(false));
    assert_eq!(
        unguarded.oriented.unguarded.len(),
        1,
        "the claim granted a licence nothing issued: {:?}",
        unguarded.oriented
    );
}

// ---------------------------------------------------------------------------
// 5. The pin under a binding frame
// ---------------------------------------------------------------------------

/// **A `bind` renames values and moves no axis, so the guard's licence carries
/// through it** — and a `reorient` under the same guard still voids it.
///
/// Both halves on one program shape, because "the pin survives everything" and
/// "the pin survives nothing" are each a way for this to be wrong, and only the
/// pair distinguishes them.
#[test]
fn a_bind_carries_the_guards_licence_and_a_reorient_still_voids_it() {
    let program = |below: fn(Node) -> Node| {
        Program::new("bind_under_guard", "piece")
            .role("mass", BlockState::simple("stone_bricks"))
            .rule("piece", reflected(Axis::Z, Node::call("inner")))
            .rule_alts(
                "inner",
                vec![
                    Alternative::new(below(fill_state("oak_stairs[facing=south,half=bottom]")))
                        .when(Cond::frame(Axis::X, Axis::Y, Axis::Z, Mirror::of(Axis::Z))),
                ],
            )
    };

    let through_bind = run(&program(|body| {
        body.with_roles([("mass", Material::block(BlockState::simple("stone")))])
    }));
    assert!(
        through_bind.oriented.carrying > 0,
        "{:?}",
        through_bind.oriented
    );
    assert!(
        through_bind.oriented.unguarded.is_empty(),
        "a bind moved no axis and must not void the licence: {:?}",
        through_bind.oriented
    );

    // A reflection on a DIFFERENT axis. It has to be a different axis: two
    // reflections of the same one cancel, which lands the body back in the very
    // frame the guard proved — where the pin holds again and the state is right
    // again, so there is correctly nothing to report there.
    let through_reorient = run(&program(|body| reflected(Axis::X, body)));
    assert_eq!(
        through_reorient.oriented.unguarded.len(),
        1,
        "a further reflection is a different frame and the guard said nothing about it: {:?}",
        through_reorient.oriented
    );

    // The cancelling case, stated rather than left to be rediscovered: two Z
    // reflections are the identity frame, the pin equals it again, and the
    // literal is what the identity frame means. Green for a reason, not by
    // omission.
    let cancelled = run(&program(|body| reflected(Axis::Z, body)));
    assert!(cancelled.oriented.carrying > 0, "{:?}", cancelled.oriented);
    assert!(
        cancelled.oriented.unguarded.is_empty(),
        "reflecting twice returns to the guarded frame: {:?}",
        cancelled.oriented
    );
}
