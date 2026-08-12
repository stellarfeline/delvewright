//! `mark`, the anchor-declaration primitive (spec-0027 phase 2b).
//!
//! An anchor is metadata: no arrangement of `fill` and `split` can say "this
//! cell is where the boss stands", and reading one back out of the blocks
//! afterwards is a guess. So the rule that shapes the space declares it. These
//! tests pin what a declaration means — which cell, which facing, which name —
//! and that every way of getting it wrong is loud.

use std::collections::BTreeMap;

use delvewright_grammar::expand::Anchor;
use delvewright_grammar::ir::{AxisSpec, Node, Reorient};
use delvewright_grammar::library::castle;
use delvewright_grammar::{
    Axis, Box3, ExpandError, ExpandOptions, Facing, Mark, MarkAt, Program, ProgramError, Side,
    expand,
};

const CASTLE_REGION: Box3 = Box3::at_origin([41, 14, 25]);

/// A program whose start rule declares `marks` and then leaves the box alone.
fn marking(marks: Vec<Mark>) -> Program {
    let body = marks
        .into_iter()
        .rev()
        .fold(Node::Skip, |body, mark| Node::Mark {
            mark,
            body: Box::new(body),
        });
    Program::new("marker", "root").rule("root", body)
}

fn anchors(program: &Program, region: Box3) -> BTreeMap<String, Anchor> {
    expand(program, region, &ExpandOptions::seeded(0))
        .unwrap()
        .anchors
}

fn at(anchors: &BTreeMap<String, Anchor>, name: &str) -> ([i32; 3], Facing) {
    let a = anchors
        .get(name)
        .unwrap_or_else(|| panic!("no anchor {name:?} in {:?}", anchors.keys()));
    (a.pos, a.facing)
}

/// Each named position means one specific cell, and the meaning is arithmetic a
/// reviewer can check rather than a snapshot. Centres round **down** on an even
/// extent — it has to be one of the two cells, and it has to be the same one
/// every time (ADR-0006).
#[test]
fn the_named_positions_land_where_they_say() {
    let program = marking(vec![
        Mark::new("corner", MarkAt::CornerMin),
        Mark::new("floor", MarkAt::FloorCenter),
        Mark::new(
            "far-face",
            MarkAt::FaceCenter {
                axis: Axis::Z,
                side: Side::Max,
            },
        ),
        Mark::new(
            "near-face",
            MarkAt::FaceCenter {
                axis: Axis::X,
                side: Side::Min,
            },
        ),
        Mark::new("explicit", MarkAt::offset(2, 1, 3)),
    ]);
    let got = anchors(&program, Box3::at_origin([7, 5, 9]));
    assert_eq!(at(&got, "anchor/corner").0, [0, 0, 0]);
    assert_eq!(
        at(&got, "anchor/floor").0,
        [3, 0, 4],
        "y is the world floor"
    );
    assert_eq!(at(&got, "anchor/far-face").0, [3, 2, 8]);
    assert_eq!(at(&got, "anchor/near-face").0, [0, 2, 4]);
    assert_eq!(at(&got, "anchor/explicit").0, [2, 1, 3]);

    // An even extent centres on the lower-middle cell, always.
    let even = anchors(
        &marking(vec![Mark::new("c", MarkAt::FloorCenter)]),
        Box3::at_origin([6, 3, 8]),
    );
    assert_eq!(at(&even, "anchor/c").0, [2, 0, 3]);
}

/// The offset and the pinned face axis are **local**: a rule that has turned its
/// box marks in the coordinates it is speaking, exactly as its splits do. The
/// floor is the one exception, and deliberately so — gravity is a world fact.
#[test]
fn offsets_are_read_through_the_scopes_orientation() {
    let inner = marking(vec![
        Mark::new("skewed", MarkAt::offset(2, 1, 3)),
        Mark::new("floor", MarkAt::FloorCenter),
    ]);
    let mut program = inner.clone();
    // `x = local Z` swaps X and Z: local X names world Z, local Z names world X.
    program.rules.insert(
        "root".to_string(),
        vec![delvewright_grammar::ir::Alternative::new(Node::Reorient {
            orient: Reorient::KEEP.x(AxisSpec::LocalZ),
            body: Box::new(
                inner
                    .rules
                    .get("root")
                    .expect("the inner program has a root")[0]
                    .body
                    .clone(),
            ),
        })],
    );

    let got = anchors(&program, Box3::at_origin([7, 5, 9]));
    assert_eq!(
        at(&got, "anchor/skewed").0,
        [3, 1, 2],
        "local x/y/z = 2/1/3 under a swapped orientation is world 3/1/2"
    );
    assert_eq!(
        at(&got, "anchor/floor").0,
        [3, 0, 4],
        "the floor does not turn with the rule"
    );
}

/// A facing nobody declared is derived from the scope's frame: the direction of
/// decreasing local `Z`. With an unreflected frame that is the negative
/// direction of the world axis the scope calls local `Z`; `tests/mirror.rs`
/// carries the reflected half, where it is the positive one.
#[test]
fn the_facing_follows_the_scope_unless_it_is_declared() {
    let plain = anchors(
        &marking(vec![Mark::new("a", MarkAt::CornerMin)]),
        Box3::at_origin([7, 5, 9]),
    );
    assert_eq!(at(&plain, "anchor/a").1, Facing::North);

    let told = anchors(
        &marking(vec![Mark::new("a", MarkAt::CornerMin).facing(Facing::East)]),
        Box3::at_origin([7, 5, 9]),
    );
    assert_eq!(at(&told, "anchor/a").1, Facing::East);

    // Turned so that local Z names world X: the derived facing turns with it.
    let mut turned = marking(vec![Mark::new("a", MarkAt::CornerMin)]);
    turned.rules.insert(
        "root".to_string(),
        vec![delvewright_grammar::ir::Alternative::new(Node::Reorient {
            orient: Reorient::KEEP.z(AxisSpec::LocalX),
            body: Box::new(Node::Mark {
                mark: Mark::new("a", MarkAt::CornerMin),
                body: Box::new(Node::Skip),
            }),
        })],
    );
    assert_eq!(
        at(&anchors(&turned, Box3::at_origin([7, 5, 9])), "anchor/a").1,
        Facing::West
    );
}

/// ...and a scope whose local Z is *vertical* has no cardinal facing at all. It
/// says so instead of inventing one.
#[test]
fn a_vertical_scope_cannot_have_its_facing_guessed() {
    let mut program = marking(vec![Mark::new("a", MarkAt::CornerMin)]);
    program.rules.insert(
        "root".to_string(),
        vec![delvewright_grammar::ir::Alternative::new(Node::Reorient {
            orient: Reorient::KEEP.z(AxisSpec::LocalY),
            body: Box::new(Node::Mark {
                mark: Mark::new("a", MarkAt::CornerMin),
                body: Box::new(Node::Skip),
            }),
        })],
    );
    let err = expand(
        &program,
        Box3::at_origin([7, 5, 9]),
        &ExpandOptions::seeded(0),
    )
    .unwrap_err();
    assert_eq!(
        err,
        ExpandError::MarkFacingNotCardinal {
            symbol: "root".to_string(),
            anchor: "a".to_string()
        }
    );
    assert!(err.to_string().contains("declare `facing`"), "{err}");

    // Declaring it is the way out, and it works.
    let mut fixed = program.clone();
    fixed.rules.insert(
        "root".to_string(),
        vec![delvewright_grammar::ir::Alternative::new(Node::Reorient {
            orient: Reorient::KEEP.z(AxisSpec::LocalY),
            body: Box::new(Node::Mark {
                mark: Mark::new("a", MarkAt::CornerMin).facing(Facing::South),
                body: Box::new(Node::Skip),
            }),
        })],
    );
    assert_eq!(
        at(&anchors(&fixed, Box3::at_origin([7, 5, 9])), "anchor/a").1,
        Facing::South
    );
}

/// Two marks may share a *cell* — a hand-built prefab routinely puts the
/// objective and the boss on the same block — but never a *name*. One name is
/// one place, and the report names both rules so the author knows where to look.
#[test]
fn two_names_on_one_cell_are_fine_and_one_name_twice_is_not() {
    let shared = anchors(
        &marking(vec![
            Mark::new("boss", MarkAt::FloorCenter),
            Mark::new("objective", MarkAt::FloorCenter),
        ]),
        Box3::at_origin([7, 5, 9]),
    );
    assert_eq!(
        at(&shared, "anchor/boss").0,
        at(&shared, "anchor/objective").0
    );

    let program = Program::new("marker", "root")
        .rule(
            "root",
            Node::Mark {
                mark: Mark::new("boss", MarkAt::CornerMin),
                body: Box::new(Node::call("again")),
            },
        )
        .rule(
            "again",
            Node::Mark {
                mark: Mark::new("boss", MarkAt::FloorCenter),
                body: Box::new(Node::Skip),
            },
        );
    let err = expand(
        &program,
        Box3::at_origin([7, 5, 9]),
        &ExpandOptions::seeded(0),
    )
    .unwrap_err();
    assert_eq!(
        err,
        ExpandError::AnchorCollision {
            anchor: "anchor/boss".to_string(),
            first: "root".to_string(),
            second: "again".to_string()
        }
    );
    assert!(err.to_string().contains("declared twice"), "{err}");
}

/// A rule that runs once per tower cannot know how many towers there are, so an
/// indexed mark numbers itself — in expansion order, from 1, per stem. That is
/// the same convention the hand-built prefabs use (`anchor/alcove-1`…).
#[test]
fn an_indexed_mark_numbers_itself_in_expansion_order() {
    // Three 3-wide slices along X, each marking its own floor centre.
    let program: Program = serde_json::from_str(
        r#"{
          "version": "1.1.0", "name": "bays", "start": "row",
          "rules": {
            "row": [{ "body": {
              "op": "split", "axis": "x",
              "sizes": [{ "size": "absolute", "blocks": { "expr": "int", "value": 3 } }],
              "repeat": true,
              "children": [{ "op": "call", "symbol": "bay" }]
            }}],
            "bay": [{ "body": {
              "op": "mark",
              "mark": { "anchor": "bay", "at": "floor_center", "index": "auto" },
              "body": { "op": "skip" }
            }}]
          }
        }"#,
    )
    .expect("the JSON authoring form carries a mark");

    let got = anchors(&program, Box3::at_origin([9, 4, 5]));
    assert_eq!(
        got.keys().collect::<Vec<_>>(),
        vec!["anchor/bay-1", "anchor/bay-2", "anchor/bay-3"]
    );
    assert_eq!(at(&got, "anchor/bay-1").0, [1, 0, 2]);
    assert_eq!(at(&got, "anchor/bay-2").0, [4, 0, 2]);
    assert_eq!(at(&got, "anchor/bay-3").0, [7, 0, 2]);

    // The numbering is a fact about the derivation, not about the run: expanding
    // again gives the same names on the same cells (ADR-0006).
    for _ in 0..3 {
        assert_eq!(anchors(&program, Box3::at_origin([9, 4, 5])), got);
    }
}

/// A rule owns its box and nothing else. Aiming a mark past the edge is the same
/// class of mistake as a split that overflows, and gets the same treatment.
#[test]
fn a_mark_outside_the_rules_own_box_is_refused() {
    for offset in [[7, 0, 0], [0, 5, 0], [0, 0, -1], [-1, 0, 0]] {
        let program = marking(vec![Mark::new(
            "over-there",
            MarkAt::offset(offset[0], offset[1], offset[2]),
        )]);
        let err = expand(
            &program,
            Box3::new([100, 64, -20], [7, 5, 9]),
            &ExpandOptions::seeded(0),
        )
        .unwrap_err();
        let ExpandError::MarkOutsideScope { anchor, cell, .. } = &err else {
            panic!("expected a refusal for offset {offset:?}, got {err}");
        };
        assert_eq!(anchor, "over-there");
        assert_eq!(*cell, [100 + offset[0], 64 + offset[1], -20 + offset[2]]);
        assert!(err.to_string().contains("outside its"), "{err}");
    }

    // A degenerate scope has no cell to mark, and says so rather than emitting
    // an anchor one block outside the piece.
    let err = expand(
        &marking(vec![Mark::new("nowhere", MarkAt::CornerMin)]),
        Box3::at_origin([7, 0, 9]),
        &ExpandOptions::seeded(0),
    )
    .unwrap_err();
    assert!(matches!(err, ExpandError::MarkOutsideScope { .. }), "{err}");
}

/// The exported key is `anchor/<kebab>` because that is the id the DSL resolves.
/// A stem that could never be one is refused where it was written, before any
/// expansion — the same treatment an unknown palette role gets.
#[test]
fn an_anchor_name_the_dsl_could_not_reference_is_refused_by_validate() {
    for bad in [
        "",
        "Boss",
        "boss stand",
        "anchor/boss",
        "-boss",
        "boss-",
        "a--b",
    ] {
        let program = marking(vec![Mark::new(bad, MarkAt::CornerMin)]);
        assert_eq!(
            program.validate(),
            Err(ProgramError::BadAnchorName {
                symbol: "root".to_string(),
                anchor: bad.to_string()
            }),
            "{bad:?} should not be a usable anchor stem"
        );
    }
    assert!(
        marking(vec![Mark::new("boss-stand-2", MarkAt::CornerMin)])
            .validate()
            .is_ok()
    );
}

/// A mark writes no blocks. The castle with its courtyard anchor is the same
/// building as the castle without it, cell for cell — which is what lets an
/// anchor be added to a ported program without the port ceasing to be faithful.
#[test]
fn declaring_an_anchor_changes_no_blocks() {
    let marked = expand(&castle(), CASTLE_REGION, &ExpandOptions::seeded(4)).unwrap();

    let mut unmarked = castle();
    let stripped = strip_marks(
        unmarked
            .rules
            .get("castle_center")
            .expect("the castle has a centre")[0]
            .body
            .clone(),
    );
    unmarked
        .rules
        .get_mut("castle_center")
        .expect("the castle has a centre")[0]
        .body = stripped;
    let plain = expand(&unmarked, CASTLE_REGION, &ExpandOptions::seeded(4)).unwrap();

    assert!(!marked.anchors.is_empty() && plain.anchors.is_empty());
    assert_eq!(
        marked.model.canonical_bytes(),
        plain.model.canonical_bytes(),
        "a mark must not move a single block"
    );
}

/// Drop every `mark` wrapper, keeping what it wrapped.
fn strip_marks(node: Node) -> Node {
    match node {
        Node::Mark { body, .. } => strip_marks(*body),
        Node::Reorient { orient, body } => Node::Reorient {
            orient,
            body: Box::new(strip_marks(*body)),
        },
        Node::Split(mut split) => {
            split.children = split.children.into_iter().map(strip_marks).collect();
            Node::Split(split)
        }
        other => other,
    }
}
