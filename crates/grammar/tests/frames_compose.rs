//! **The three frame-shaped constructs, composed** — the tests that could not
//! exist on any one branch.
//!
//! A scope is a box, a set of axis names and a set of value names, and the
//! language now has one construct per part: `split` narrows the box, `reorient`
//! renames the axes *and carries their direction*, `bind` rebinds the values.
//! `claim` names the box for the contract. Each arrived separately, so each was
//! proved alone, and an integration is the first place their interaction exists
//! at all.
//!
//! Three pairs and one triple, and what each would look like going wrong:
//!
//! 1. **`claim` under `bind`.** A claim inside a rebound frame has to be seen by
//!    the walk that collects claims, and its box has to be the box the rebound
//!    parameters produced. Missed, the contract either refuses a region the
//!    program does declare or records the box the *default* would have made.
//! 2. **`claim` under a reflection.** A reflected rule claims the reflected box,
//!    or a contract describes one arm of a mirror pair and silently mis-places
//!    the other.
//! 3. **`bind` under a reflection.** A binding is evaluated in the scope it is
//!    written in, and a reflection reverses that scope's axes without moving its
//!    extents — so an expression over a dimension reads the same reflected as
//!    not, or one arm of a symmetric pair is built to a different size than the
//!    rule asked for.
//! 4. **The triple.** A caller pushes a name frame into a mirrored rule that
//!    claims a named box: the blocks are the mirror image, the claimed boxes are
//!    the mirror image, and the pushed value is what sized both.

use delvewright_grammar::block::BlockState;
use delvewright_grammar::gates;
use delvewright_grammar::geom::{Axis, Box3};
use delvewright_grammar::ir::{
    Contract, EXTERIOR, EdgeClass, Envelope, Expr, Node, Program, Reorient,
};
use delvewright_grammar::{ExpandOptions, Expansion, expand};

/// A hall wide enough for two arms of four with a two-cell band in each.
const HALL: [u32; 3] = [12, 4, 4];

fn run(program: &Program, size: [u32; 3], seed: u64) -> Expansion {
    program
        .validate()
        .unwrap_or_else(|e| panic!("{}: {e}", program.name));
    expand(program, Box3::at_origin(size), &ExpandOptions::seeded(seed))
        .unwrap_or_else(|e| panic!("{}: {e}", program.name))
}

/// One asymmetric arm, written **once**.
///
/// A band of `band` cells at the arm's own local `X` minimum, then the room —
/// claimed, so the contract learns where it is. Nothing about this rule knows
/// which side of the hall it stands on, what `band` is, or whether its frame is
/// reflected: all three are the caller's, which is the whole point.
fn arm() -> Node {
    Node::Split(delvewright_grammar::ir::Split {
        axis: Axis::X,
        sizes: vec![
            delvewright_grammar::ir::Size::Absolute {
                blocks: Expr::param("band"),
            },
            delvewright_grammar::ir::Size::Relative {
                weight: Expr::int(1),
            },
        ],
        rounding: delvewright_grammar::ir::Rounding::Truncate,
        repeat: false,
        orient: Reorient::KEEP,
        children: vec![
            Node::fill("band"),
            Node::Claim {
                region: "room".to_string(),
                body: Box::new(Node::Void),
            },
        ],
    })
}

/// A hall of two arms, each `arm()` under its own frame.
///
/// `left_band` / `right_band` are what each side's `bind` pushes; `reflect_right`
/// is whether the right arm's frame is reflected. Every combination this file
/// needs is one call of this.
fn hall(left_band: i64, right_band: i64, reflect_right: bool) -> Program {
    let right = Node::call("arm").with_params([("band", Expr::int(right_band))]);
    let right = if reflect_right {
        Node::Reorient {
            orient: Reorient::KEEP.flip(Axis::X),
            body: Box::new(right),
        }
    } else {
        right
    };
    Program::new("hall", "all")
        .param("band", 1)
        .role("band", BlockState::simple("minecraft:stone_bricks"))
        .rule(
            "all",
            Node::Split(delvewright_grammar::ir::Split {
                axis: Axis::X,
                sizes: vec![
                    delvewright_grammar::ir::Size::Relative {
                        weight: Expr::int(1),
                    },
                    delvewright_grammar::ir::Size::Relative {
                        weight: Expr::int(1),
                    },
                ],
                rounding: delvewright_grammar::ir::Rounding::Truncate,
                repeat: false,
                orient: Reorient::KEEP,
                children: vec![
                    Node::call("arm").with_params([("band", Expr::int(left_band))]),
                    right,
                ],
            }),
        )
        .rule("arm", arm())
        .contract(Contract::new("room").space("room", Envelope::Open).edge(
            "room",
            EXTERIOR,
            EdgeClass::Walk { rise: 0, via: None },
        ))
}

/// The boxes one region resolved to, in the contract's own canonical order.
fn boxes(out: &Expansion, region: &str) -> Vec<Box3> {
    out.contract
        .as_ref()
        .unwrap_or_else(|| panic!("the expansion carries no resolved contract"))
        .spaces
        .get(region)
        .unwrap_or_else(|| panic!("no space {region:?}"))
        .region
        .boxes
        .clone()
}

/// The box `b` reflected across the mid-plane of a `width`-wide hall.
fn reflect_x(b: Box3, width: u32) -> Box3 {
    Box3::new(
        [
            width as i32 - b.origin[0] - b.size[0] as i32,
            b.origin[1],
            b.origin[2],
        ],
        b.size,
    )
}

// ---------------------------------------------------------------------------
// 1. `claim` under `bind`
// ---------------------------------------------------------------------------

/// **A resolved contract region sees the rebound parameters**, and a claim under
/// a frame is seen at all.
///
/// The pair #418 named for the merge. `arm`'s room is whatever the band left
/// over, and the band is the caller's — so the same rule, called under two
/// frames, claims two differently sized boxes in one expansion. Both halves are
/// asserted: that the claim is *found* (a walk that did not descend through
/// `bind` would leave the region unclaimed, and `validate` would refuse the
/// program outright), and that the box is the one the pushed value made.
#[test]
fn a_claim_under_a_pushed_frame_resolves_to_the_box_that_frame_made() {
    let out = run(&hall(2, 3, false), HALL, 0);
    let got = boxes(&out, "room");
    assert_eq!(
        got.len(),
        2,
        "one rule claimed under two frames must resolve to two boxes, got {got:?}"
    );
    // Left arm x∈[0,6) with band 2 → room x∈[2,6). Right arm x∈[6,12) with
    // band 3 → room x∈[9,12). The widths ARE the pushed values, read back off
    // the contract rather than off the blocks.
    assert_eq!(got[0], Box3::new([2, 0, 0], [4, 4, 4]), "left room");
    assert_eq!(got[1], Box3::new([9, 0, 0], [3, 4, 4]), "right room");
    assert_ne!(
        got[0].size[0], got[1].size[0],
        "the two frames pushed different values and the boxes must differ, or this \
         test would pass against an engine that ignored the frame entirely"
    );
}

/// **A frame over a claim moves no block and no box.**
///
/// The same program with the two `bind`s replaced by the values they push: same
/// blocks, same resolved contract. A `bind` is a scope change, and a scope change
/// that moved a claimed box would make every contract a function of how the
/// author happened to wrap their rules.
#[test]
fn rebinding_a_name_to_the_value_it_already_has_changes_neither_blocks_nor_boxes() {
    let framed = hall(2, 2, false);
    // The same hall with no frame anywhere: `band` is the program default, and
    // the default is the value both frames were pushing.
    let plain = Program::new("hall", "all")
        .param("band", 2)
        .role("band", BlockState::simple("minecraft:stone_bricks"))
        .rule(
            "all",
            Node::Split(delvewright_grammar::ir::Split {
                axis: Axis::X,
                sizes: vec![
                    delvewright_grammar::ir::Size::Relative {
                        weight: Expr::int(1),
                    },
                    delvewright_grammar::ir::Size::Relative {
                        weight: Expr::int(1),
                    },
                ],
                rounding: delvewright_grammar::ir::Rounding::Truncate,
                repeat: false,
                orient: Reorient::KEEP,
                children: vec![Node::call("arm"), Node::call("arm")],
            }),
        )
        .rule("arm", arm())
        .contract(Contract::new("room").space("room", Envelope::Open).edge(
            "room",
            EXTERIOR,
            EdgeClass::Walk { rise: 0, via: None },
        ));
    for seed in [0u64, 1, 7] {
        let a = run(&framed, HALL, seed);
        let b = run(&plain, HALL, seed);
        assert_eq!(
            a.model.canonical_bytes(),
            b.model.canonical_bytes(),
            "seed {seed}"
        );
        assert_eq!(boxes(&a, "room"), boxes(&b, "room"), "seed {seed}");
    }
    assert!(
        !boxes(&run(&framed, HALL, 0), "room").is_empty(),
        "binding count 0: the comparison above compared two empty contracts"
    );
}

// ---------------------------------------------------------------------------
// 2. `claim` under a reflection
// ---------------------------------------------------------------------------

/// **A reflected rule claims the reflected box.**
///
/// One rule at both sides of the hall's mirror plane, the same value pushed to
/// each: the blocks are symmetric — `--symmetric x`, the gate #413 added, says so
/// with its binding count — and so are the two boxes the contract resolved. A
/// `claim` that recorded the unreflected box would leave the gate green and the
/// contract wrong, which is the exact shape a contract exists to stop.
#[test]
fn a_mirrored_rule_claims_the_mirror_image_box_and_the_blocks_agree() {
    let out = run(&hall(2, 2, true), HALL, 0);
    let got = boxes(&out, "room");
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!(
        got[1],
        reflect_x(got[0], HALL[0]),
        "the reflected arm's room must be the plain arm's room, mirrored"
    );

    let report = gates::judge(
        &out,
        gates::Options {
            traversable: false,
            allow_falls: false,
            symmetric: Some(Axis::X),
            reachable_floor: false,
        },
    );
    let symmetric = report
        .gates
        .iter()
        .find(|g| g.id.contains("symmetric"))
        .expect("the symmetry gate ran");
    assert!(symmetric.pass, "{}", symmetric.detail);
    assert!(
        symmetric.bound > 0,
        "the symmetry gate examined zero cell pairs — {}",
        symmetric.detail
    );
    // This hall's contract is deliberately the smallest one that can carry a
    // claim: one `open` space, one way out, no floor. Every obligation that has
    // nothing to look at is therefore RED, not quietly green — closure has no
    // envelope to examine, no edge runs between two spaces, and no declared
    // space holds a cell to stand in. That is the spec-0036 §2.9 vacuity rule
    // doing its job on a fixture that is about the mirror, not about the
    // building, and asserting it here is what stops the rule being softened the
    // next time a fixture trips it.
    let red: Vec<&str> = report
        .gates
        .iter()
        .filter(|g| !g.pass)
        .map(|g| g.id)
        .collect();
    assert_eq!(
        red,
        vec!["contract-closure", "contract-reachability"],
        "{:#?}",
        report.gates
    );
    assert!(
        report
            .gates
            .iter()
            .filter(|g| !g.pass)
            .all(|g| g.bound == 0),
        "every red here is a zero binding, not a disagreement about geometry"
    );

    // The falsifiable direction: without the reflection the same program is not
    // symmetric, and the boxes are translations rather than reflections.
    let lopsided = run(&hall(2, 2, false), HALL, 0);
    let flat = boxes(&lopsided, "room");
    assert_ne!(flat[1], reflect_x(flat[0], HALL[0]));
    assert!(
        !gates::judge(
            &lopsided,
            gates::Options {
                traversable: false,
                allow_falls: false,
                symmetric: Some(Axis::X),
                reachable_floor: false,
            },
        )
        .is_pass(),
        "the unreflected hall must fail the symmetry gate, or the gate proves nothing"
    );
}

// ---------------------------------------------------------------------------
// 3. `bind` under a reflection
// ---------------------------------------------------------------------------

/// **A binding is evaluated in the scope it is written in, and a reflection does
/// not move that scope's extents.**
///
/// The band is bound to an expression over the arm's own width rather than to a
/// literal, and the arm is expanded plain and reflected. `grammar.md` states that
/// a reflected scope is exactly as wide as its mirror image; if that were false
/// for the purpose of an expression, every size written for one half of a
/// symmetric shape would be wrong in the other half — silently, because both
/// halves would still be *some* building.
#[test]
fn a_binding_reads_the_same_extents_under_a_reflection_as_without_one() {
    fn one_arm(reflect: bool) -> Program {
        let body = Node::call("arm").with_params([(
            "band",
            Expr::dim(delvewright_grammar::ir::DimRef::X)
                .arith(delvewright_grammar::ir::ArithOp::Div, Expr::int(3)),
        )]);
        let body = if reflect {
            Node::Reorient {
                orient: Reorient::KEEP.flip(Axis::X),
                body: Box::new(body),
            }
        } else {
            body
        };
        Program::new("one-arm", "all")
            .param("band", 1)
            .role("band", BlockState::simple("minecraft:stone_bricks"))
            .rule("all", body)
            .rule("arm", arm())
            .contract(Contract::new("room").space("room", Envelope::Open).edge(
                "room",
                EXTERIOR,
                EdgeClass::Walk { rise: 0, via: None },
            ))
    }
    let plain = run(&one_arm(false), HALL, 0);
    let flipped = run(&one_arm(true), HALL, 0);
    let a = boxes(&plain, "room");
    let b = boxes(&flipped, "room");
    assert_eq!(a.len(), 1);
    assert_eq!(b.len(), 1);
    // 12 / 3 = 4 either way: the band is four cells wide in both, at opposite
    // ends. A reflection that reversed the extent as well as the direction would
    // give a band of some other width here.
    assert_eq!(
        a[0].size, b[0].size,
        "the reflected arm is a different size"
    );
    assert_eq!(a[0], Box3::new([4, 0, 0], [8, 4, 4]));
    assert_eq!(b[0], reflect_x(a[0], HALL[0]));
}

// ---------------------------------------------------------------------------
// 4. The triple
// ---------------------------------------------------------------------------

/// **A caller pushes a name frame into a mirrored rule that claims a named box.**
///
/// All three at once, and each is load-bearing in the assertion:
///
/// * the *frame* decides the band, so the two arms are different where the two
///   pushed values differ;
/// * the *reflection* decides which end of each arm the band sits at, so both
///   bands are on the hall's outside;
/// * the *claim* is what reports it, resolved per arm, and the two boxes are
///   what a later obligation would read.
///
/// The counter-case is the one the mirror primitive exists for: with the
/// reflection removed and everything else identical, both bands sit at the same
/// end and one of them is inboard.
#[test]
fn a_pushed_frame_a_reflection_and_a_claim_compose_in_one_expansion() {
    let out = run(&hall(2, 3, true), HALL, 0);
    let got = boxes(&out, "room");
    assert_eq!(got.len(), 2, "{got:?}");
    // Left arm x∈[0,6), band 2 at its low end → room x∈[2,6).
    // Right arm x∈[6,12) reflected, band 3 at its own low end, which is the
    // world HIGH end → room x∈[6,9).
    assert_eq!(got[0], Box3::new([2, 0, 0], [4, 4, 4]), "left room");
    assert_eq!(got[1], Box3::new([6, 0, 0], [3, 4, 4]), "right room");

    // Both bands are on the hall's outside: the outermost column of each end is
    // band, and the columns just inside the rooms are not.
    let band = |x: i32| {
        out.model
            .get([x, 0, 0])
            .map(|b| !b.is_air())
            .unwrap_or(false)
    };
    assert!(
        band(0) && band(1),
        "the left band is two cells at the west face"
    );
    assert!(
        band(9) && band(10) && band(11),
        "the right band is three cells at the east face"
    );
    assert!(
        (2..=8).all(|x| !band(x)),
        "everything between the two bands is room, and a room is hollow"
    );

    // Without the reflection, the right arm's band moves inboard — the defect
    // the reflection exists to remove, measured rather than described.
    let unreflected = run(&hall(2, 3, false), HALL, 0);
    let inboard = unreflected
        .model
        .get([6, 0, 0])
        .map(|b| !b.is_air())
        .unwrap_or(false);
    assert!(
        inboard,
        "without the reflection the right band should sit against the partition"
    );

    // And the claim follows the blocks in both cases rather than either one
    // being computed independently: the boxes differ exactly where the models do.
    assert_ne!(boxes(&unreflected, "room"), got);
}

/// Each construct is refused by its own fence, and the document that writes all
/// three is refused by every version below the newest of them.
///
/// Two claims, and the second needs its own documents: `validate` stops at the
/// first fenced construct it reaches, so one program crossing three fences
/// reports one refusal and could never show which fence answers for which
/// construct. That is what the minimal programs below are for.
#[test]
fn every_fence_this_document_crosses_refuses_it_at_the_version_below() {
    use delvewright_grammar::ir::ProgramError;
    use delvewright_grammar::version::{BIND_SINCE, CONTRACT_SINCE, MIRROR_SINCE};

    let all_three = hall(2, 3, true);
    assert!(all_three.validate().is_ok());
    for version in ["1.0.0", "1.1.0", "1.2.0"] {
        assert!(
            matches!(
                all_three.clone().at_version(version).validate(),
                Err(ProgramError::FencedConstruct { .. })
            ),
            "a document writing a reflection, a claim and a binding must be refused at \
             {version}"
        );
    }

    // One construct per document, so the fence that answers is the construct's
    // own. Each is written at the version just below its fence: the fence is
    // what refuses it, not the floor.
    let stone = || BlockState::simple("minecraft:stone_bricks");
    let base = || {
        Program::new("one", "all")
            .param("band", 1)
            .role("band", stone())
    };
    let cases: [(&str, Program, &str, &str); 3] = [
        (
            "1.0.0",
            base().rule(
                "all",
                Node::Reorient {
                    orient: Reorient::KEEP.flip(Axis::X),
                    body: Box::new(Node::fill("band")),
                },
            ),
            MIRROR_SINCE,
            "reflect",
        ),
        (
            "1.1.0",
            base()
                .rule(
                    "all",
                    Node::Claim {
                        region: "room".to_string(),
                        body: Box::new(Node::fill("band")),
                    },
                )
                .contract(Contract::new("room").space("room", Envelope::Open).edge(
                    "room",
                    EXTERIOR,
                    EdgeClass::Walk { rise: 0, via: None },
                )),
            CONTRACT_SINCE,
            "claim",
        ),
        (
            "1.2.0",
            base().rule(
                "all",
                Node::fill("band").with_params([("band", Expr::int(2))]),
            ),
            BIND_SINCE,
            "bind",
        ),
    ];
    let mut checked = 0usize;
    for (declared, program, fence, word) in cases {
        assert!(
            program.validate().is_ok(),
            "{word}: the program must be legal at the current version first"
        );
        match program.at_version(declared).validate() {
            Err(ProgramError::FencedConstruct {
                construct, since, ..
            }) => {
                assert_eq!(since, fence, "{word} answered to the wrong fence");
                assert!(
                    construct.contains(word),
                    "the refusal must name the construct: {construct:?} does not say {word:?}"
                );
                checked += 1;
            }
            other => panic!("{word} at {declared} must be refused, got {other:?}"),
        }
    }
    assert_eq!(
        checked, 3,
        "one document per fence, and every fence answered"
    );
}
