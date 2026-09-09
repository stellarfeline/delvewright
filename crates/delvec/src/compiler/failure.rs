//! The one type a compiler pass fails with.
//!
//! # Why there is exactly one
//!
//! A pass that refuses has two things to say and no more: **which rule refused**
//! — a stable [`DwCode`], the name the catalog, the fence and the exit map all
//! read it by — and **what the author does about it**, in prose. Every geometric
//! proof, every replay check and every emitted-tree audit in this crate had
//! independently declared that pair as its own private struct: twelve of them,
//! field-for-field identical down to the doc comments, plus two more carrying
//! one extra field each.
//!
//! Twelve copies of a two-field record is not merely repetition. Each copy is a
//! type, so every boundary between two passes needed a conversion — a `From`
//! impl, or a closure spelled out at the call site, mapping one module's
//! two-field record onto another's — and each of those is a place a
//! message or a code can be dropped, rewritten or defaulted with nothing to
//! notice. And a capability the pair should carry has to be added twelve times
//! or it is missing eleven times.
//!
//! So the pair is one type. A pass returns `Result<_, Failure>`; a boundary
//! between passes is `?`; nothing converts, so nothing can convert wrongly.
//!
//! # What this is not
//!
//! It is **not** a [`Diagnostic`](delvewright_dsl::Diagnostic). A `Diagnostic`
//! is a reportable finding placed in a document — it carries a severity, a
//! stage and a path, and a whole run's worth of them are printed together. A
//! `Failure` is the single thing that stopped a pass dead: it has no severity
//! (a failure is an error by construction — that is what makes it a failure),
//! and it has no document position, because the passes that raise it read the
//! assembled world, the solved layout and the emitted tree rather than the
//! campaign's JSON. `main.rs` renders one through `print_build_error`, which is
//! where it acquires the `stage: "build"` a `--json` consumer sees.

use delvewright_dsl::DwCode;

/// A hard failure raised by a compiler pass: the stable DW code naming the rule
/// that refused, and the message saying what to do about it.
///
/// The message is written in the remediation-contract style the rest of the
/// compiler uses — it names the offending object, where it is, and the fix —
/// because it is the entire text the author gets.
#[derive(Debug, Clone)]
pub struct Failure {
    /// The stable diagnostic code.
    pub code: DwCode,
    /// Human-readable explanation, naming the object, its place and the fix.
    pub message: String,
}

impl Failure {
    /// Raise a failure under `code`.
    pub fn new(code: DwCode, message: impl Into<String>) -> Self {
        Failure {
            code,
            message: message.into(),
        }
    }
}

/// How many offending cells a floor names before the message says "and N more".
///
/// The list is what an author walks to; the count is what tells them how much of
/// the floor is affected. Both are needed, and a message that printed 400 cells
/// would be neither.
pub const SHOWN_PER_FLOOR: usize = 6;

/// **A set of offending cells, as a reader walks it: grouped by floor, six named
/// per floor, then a count.**
///
/// One formatter, because two rules print the same set about the same world.
/// `DW0881` says which floor cells complete an objective they cannot walk to;
/// `DW0891` (spec-0062 §4) says which floor cells a killing volume catches
/// without showing it. A reader who has read one is reading the same shape in
/// the other, and a second copy would be free to drift from it.
///
/// Deterministic: a `BTreeMap` keyed by `y`, and the cells in the order the
/// caller enumerated them (ADR-0006).
pub fn cells_by_floor(cells: &[[i32; 3]]) -> String {
    let mut by_y: std::collections::BTreeMap<i32, Vec<[i32; 3]>> =
        std::collections::BTreeMap::new();
    for c in cells {
        by_y.entry(c[1]).or_default().push(*c);
    }
    by_y.iter()
        .map(|(y, cs)| {
            let shown = cs
                .iter()
                .take(SHOWN_PER_FLOOR)
                .map(|c| format!("{c:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            let rest = cs.len().saturating_sub(SHOWN_PER_FLOOR);
            if rest == 0 {
                format!("y={y} ({} cell(s): {shown})", cs.len())
            } else {
                format!(
                    "y={y} ({} cell(s): {shown}, and {rest} more on the same floor)",
                    cs.len()
                )
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}
