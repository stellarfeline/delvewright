//! **A struct field rides through in silence; a variant does not.**
//!
//! `Node`, `Cond`, `Expr`, `Size` and `MarkAt` are internally tagged, so an
//! engine that predates a new variant meets an `"op"` it does not know and fails
//! loud at serde — and every exhaustive `match` in the crate forces an arm for
//! it. A `#[serde(default)]` **struct field** has neither property: it rides
//! through every walk untouched in both directions.
//!
//! Measured, on three real engines and one document. The same 15x11x2 program,
//! `reorient.mirror` on its far half:
//!
//! ```text
//! origin/main (pre-mirror)   exit 0   d584632…   the UNREFLECTED building
//!                                     blocks-exist pass, non-empty pass, verdict `pass`
//! this branch, no fence      exit 0   73be657…   the reflected building
//! this branch, `1.1.0`       exit 0   73be657…
//! this branch, `1.0.0`       exit 2   refused    ProgramError::FencedConstruct
//! ```
//!
//! Two different buildings from one file, and the engine that built the wrong one
//! had nothing to say — `--symmetric` is this branch's own flag, so on
//! `origin/main` there was not even a gate that could have looked.
//!
//! The fence cannot reach an engine older than itself; what it does is make
//! every version from `1.1.0` on refuse rather than reinterpret. This file is
//! what binds that to the corpus rather than to a doc line.

use std::collections::BTreeMap;

use delvewright_grammar::block::BlockState;
use delvewright_grammar::geom::{Axis, Mirror};
use delvewright_grammar::ir::{Alternative, Cond, Node, Program, ProgramError, Reorient};
use delvewright_grammar::library;
use delvewright_grammar::version::{
    BIND_SINCE, CONTRACT_SINCE, LATEST_PROGRAM_VERSION, MIRROR_SINCE, SUPPORTED_PROGRAM_VERSIONS,
    has_mirror,
};

/// A one-rule program whose body carries the frame request it is given.
fn with_body(body: Node) -> Program {
    Program::new("fence", "all")
        .role("stone", BlockState::simple("minecraft:stone"))
        .rule_alts("all", vec![Alternative::new(body)])
}

fn reflected_reorient() -> Node {
    Node::Reorient {
        orient: Reorient::default().mirror(Mirror::of(Axis::X)),
        body: Box::new(Node::fill("stone")),
    }
}

/// A document declaring a version that predates `mirror` is refused where the
/// reflection is written, not half-built.
#[test]
fn a_reflected_frame_request_is_refused_below_its_version() {
    let program = with_body(reflected_reorient()).at_version("1.0.0");
    match program.validate() {
        Err(ProgramError::FencedConstruct {
            construct,
            since,
            declared,
            written_by,
        }) => {
            assert!(construct.contains("mirror"), "{construct}");
            assert_eq!(since, MIRROR_SINCE);
            assert_eq!(declared, "1.0.0");
            assert!(written_by.contains("all"), "{written_by}");
        }
        other => panic!("expected a fenced-construct refusal, got {other:?}"),
    }
    // The same program at the version that introduced it is fine, so the fence
    // refuses the declaration and not the construct.
    assert!(with_body(reflected_reorient()).validate().is_ok());
}

/// The frame *guard* is fenced too. It is the construct an author reaches for to
/// keep a stair from being placed backwards in a reflected scope, so an engine
/// that dropped it would choose the wrong alternative rather than fail.
#[test]
fn a_reflected_frame_guard_is_refused_below_its_version() {
    let guarded = Program::new("fence", "all")
        .role("stone", BlockState::simple("minecraft:stone"))
        .rule_alts(
            "all",
            vec![
                Alternative::new(Node::fill("stone")).when(Cond::frame(
                    Axis::X,
                    Axis::Y,
                    Axis::Z,
                    Mirror::of(Axis::X),
                )),
                Alternative::new(Node::Void).when(Cond::Otherwise),
            ],
        );
    assert!(guarded.validate().is_ok());
    match guarded.at_version("1.0.0").validate() {
        Err(ProgramError::FencedConstruct {
            construct, since, ..
        }) => {
            assert!(construct.contains("mirror"), "{construct}");
            assert_eq!(since, MIRROR_SINCE);
        }
        other => panic!("expected a fenced-construct refusal, got {other:?}"),
    }
}

/// An unreflected frame request and an unreflected guard are `1.0.0` surface and
/// stay writable there forever — the fence is on the reflection, not on the
/// construct that carries it.
#[test]
fn the_fence_is_on_the_reflection_and_not_on_the_frame() {
    let plain = with_body(Node::Reorient {
        orient: Reorient::default().x(delvewright_grammar::ir::AxisSpec::WorldZ),
        body: Box::new(Node::fill("stone")),
    })
    .at_version("1.0.0");
    assert!(plain.validate().is_ok(), "{:?}", plain.validate());

    let plain_guard = Program::new("fence", "all")
        .role("stone", BlockState::simple("minecraft:stone"))
        .rule_alts(
            "all",
            vec![
                Alternative::new(Node::fill("stone")).when(Cond::orientation(
                    Axis::X,
                    Axis::Y,
                    Axis::Z,
                )),
            ],
        )
        .at_version("1.0.0");
    assert!(plain_guard.validate().is_ok());
}

/// A version this engine does not know is refused entire, rather than parsed for
/// the parts it recognises — which is the failure mode the fence exists for, one
/// level up.
#[test]
fn an_unknown_version_is_refused_rather_than_parsed_best_effort() {
    for unknown in ["0.9.0", "1.4.0", "2.0.0", "", "latest"] {
        let program = with_body(Node::fill("stone")).at_version(unknown);
        match program.validate() {
            Err(ProgramError::UnsupportedVersion { version }) => assert_eq!(version, unknown),
            other => panic!("{unknown:?} should be refused, got {other:?}"),
        }
        assert!(
            !has_mirror(unknown),
            "an unknown version must enable nothing"
        );
    }
}

/// An unknown FIELD is refused by name, on every document type serde can close.
///
/// This is the half that needs no author to remember anything: it is what makes
/// the *next* optional field safe without a fence being written for it. The one
/// type it cannot cover is `mark`, whose `at` is a flattened sum.
#[test]
fn an_unknown_field_is_refused_by_name_not_dropped() {
    let cases = [
        (
            "a program-level field",
            r#"{"version":"1.1.0","name":"n","start":"all","from_the_future":true,
                "rules":{"all":[{"body":{"op":"void"}}]}}"#,
        ),
        (
            "a field of a frame request",
            r#"{"version":"1.1.0","name":"n","start":"all","rules":{"all":[{"body":
                {"op":"reorient","orient":{"shear":true},"body":{"op":"void"}}}]}}"#,
        ),
        (
            "a field of a split",
            r#"{"version":"1.1.0","name":"n","start":"all","rules":{"all":[{"body":
                {"op":"split","axis":"x","stagger":2,
                 "sizes":[{"size":"relative","weight":1}],
                 "children":[{"op":"void"}]}}]}}"#,
        ),
        (
            "a field of an alternative",
            r#"{"version":"1.1.0","name":"n","start":"all","rules":{"all":[
                {"body":{"op":"void"},"priority":3}]}}"#,
        ),
        (
            "a field of a reflection",
            r#"{"version":"1.1.0","name":"n","start":"all","rules":{"all":[{"body":
                {"op":"reorient","orient":{"mirror":{"x":true,"w":true}},
                 "body":{"op":"void"}}}]}}"#,
        ),
    ];
    for (what, json) in cases {
        let err = serde_json::from_str::<Program>(json)
            .expect_err(&format!("{what} must be refused, not dropped"));
        assert!(err.to_string().contains("unknown field"), "{what}: {err}");
    }
    assert_eq!(cases.len(), 5, "five document types exercised");
}

/// **The corpus binding.** Every library program declares a version this engine
/// accepts and validates under it; and every program that writes a construct a
/// fence guards is refused when its version is lowered below every fence.
///
/// One sweep, three fences. Lowering to `1.0.0` is "the surface the format had
/// before any of this", so a refusal names whichever fenced construct the
/// program reached first, and the tally is per fence. Every count is printed and
/// every count is asserted non-zero: a fence with no library program behind it
/// is proved only on a hand-written fixture, which is the binding this file
/// exists to supply.
#[test]
fn the_fence_binds_to_the_corpus_and_not_only_to_a_fixture() {
    let mut declared = 0usize;
    let mut refused: BTreeMap<&str, usize> = BTreeMap::new();
    for (id, build) in library::PROGRAMS {
        let program = build();
        assert!(
            SUPPORTED_PROGRAM_VERSIONS.contains(&program.version.as_str()),
            "{id} declares {:?}, which this engine does not accept",
            program.version
        );
        assert!(program.validate().is_ok(), "{id}: {:?}", program.validate());
        declared += 1;

        let lowered = build().at_version("1.0.0");
        match lowered.validate() {
            Err(ProgramError::FencedConstruct { since, .. }) => {
                assert!(
                    [MIRROR_SINCE, CONTRACT_SINCE, BIND_SINCE].contains(&since),
                    "{id} was refused at a fence this sweep does not know: {since}"
                );
                *refused.entry(since).or_default() += 1;
            }
            Ok(()) => {}
            other => panic!("{id} failed for an unrelated reason: {other:?}"),
        }
    }
    println!(
        "version fence   bound {declared:3}  library program(s) at {LATEST_PROGRAM_VERSION}; \
         refused at 1.0.0 by fence: {refused:?}"
    );
    assert!(declared > 0, "the corpus sweep examined zero programs");
    for fence in [MIRROR_SINCE, CONTRACT_SINCE, BIND_SINCE] {
        assert!(
            refused.get(fence).copied().unwrap_or(0) > 0,
            "binding count 0 for the fence at {fence}: ZERO library programs write its \
             construct, so lowering the version refuses nothing on its account and this \
             sweep is a green that binds to nothing for it"
        );
    }
}
