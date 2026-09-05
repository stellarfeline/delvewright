//! **A corpus example, not an idiom-index entry** (spec-0033 §4.8): the one
//! program in the library that writes `none_of`.
//!
//! The idiom index ([`super::idioms`]) is a curated set of *techniques* and
//! grows only when an authoring trial fails for want of one. This is the other
//! set: every IR construct owes the corpus at least one example an author can
//! reach with `delvec grammar show`, because the corpus is what an author reads
//! instead of the schema. Negating a guard is a language feature, not a way of
//! building anything, so it earns an example here and no entry there.
//!
//! # What `none_of` is
//!
//! `none_of` holds when **no** sub-guard does — the exact complement of
//! `any_of`, and the shape of a sentence that starts with *unless*. This rule
//! buttresses a pier unless the box is too thin or too short for buttresses to
//! mean anything:
//!
//! ```json
//! { "cond": "none_of", "of": [
//!     { "cond": "cmp", "lhs": {"expr":"dim","dim":"x"}, "op": "lt",
//!       "rhs": {"expr":"param","name":"min_thick"} },
//!     { "cond": "cmp", "lhs": {"expr":"dim","dim":"z"}, "op": "lt",
//!       "rhs": {"expr":"param","name":"min_run"} } ] }
//! ```
//!
//! Written the other way it is an `all_of` of two `ge` comparisons, and the two
//! are the same guard. Which one to write is a question of which sentence the
//! rule is really making: `all_of` states requirements, `none_of` states
//! disqualifications, and a reader can tell them apart.
//!
//! **`none_of` is not `otherwise`.** `otherwise` is not a guard at all — it is
//! the arm that runs when no *other alternative* matched, and it cannot look at
//! the scope. `none_of` is an ordinary guard that happens to be negative, so it
//! composes: it can sit inside an `all_of`, be one arm of an `any_of`, or guard
//! an alternative that has no fallback at all.
//!
//! Smallest region that expands: any. Under the disqualifying sizes the piece is
//! a plain pier rather than a refusal, which is the point of pairing the guard
//! with an `otherwise`.
//!
//! Documented at **5 × 4 × 12, seed 1** (buttressed), and at **2 × 4 × 12**,
//! where the first sub-guard holds and the pier is left plain.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{CmpOp, Cond, DimRef, Program};

use super::{abs, alt_else, alt_when, call, cmp, dim, fill, par, rel, split};

/// A pier that is buttressed unless its box disqualifies it.
///
/// Controls: `min_thick` (3), `min_run` (9). Roles: `mass`, `buttress`.
pub fn negated_guard() -> Program {
    Program::new("negated_guard", "pier")
        .param("min_thick", 3)
        .param("min_run", 9)
        .role("mass", BlockState::simple("stone_bricks"))
        .role("buttress", BlockState::simple("polished_andesite"))
        .rule_alts(
            "pier",
            vec![
                alt_when(
                    Cond::NoneOf {
                        of: vec![
                            cmp(dim(DimRef::X), CmpOp::Lt, par("min_thick")),
                            cmp(dim(DimRef::Z), CmpOp::Lt, par("min_run")),
                        ],
                    },
                    call("buttressed"),
                ),
                alt_else(fill("mass")),
            ],
        )
        .rule(
            "buttressed",
            split(
                Axis::Z,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("buttress"), fill("mass"), fill("buttress")],
            ),
        )
}
