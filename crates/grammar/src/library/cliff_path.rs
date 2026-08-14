//! The knockback niche — a one-wide ledge with ambush recesses cut into the
//! inner wall (W1 entry K, drowned-bell remake step 2).
//!
//! **Original Delvewright content**, not a port: nothing upstream, licence
//! `original`. What it builds is not a building at all but an *encounter shape*
//! — the oldest trick in the souls vocabulary, a path narrow enough that being
//! shoved is the whole threat, with something waiting where you cannot see it.
//!
//! ```text
//!  local X:   0        1        2..            travel: local Z-max -> Z-min
//!            ledge   recess   backing
//!  y=3+      air     ######   ######           <- lintel over the recess
//!  y=1..2    air     air/###  ######           <- the niche band: the Z-varying course
//!  y=0       ######  ######   ######           <- floor
//!            ^ the drop face is the box's X-min face; the void is beyond it
//! ```
//!
//! Only the **niche band** varies along the path. Everything else is two flat
//! courses, which is what keeps the rule small enough to reason about: the
//! recursion that spaces the recesses runs on a 2-block-high slice and nothing
//! else.
//!
//! # The two gates
//!
//! 1. **The recess is exactly one deep.** An occupant's hitbox sits inside it
//!    and a swing from the ledge reaches it; one deeper and the niche becomes a
//!    room the player has to walk into, which is a different (and worse) fight.
//! 2. **The ledge is the only route.** A recess beside a *wide* path is
//!    decoration. The rule leaves exactly one walkable lane, along the drop
//!    face, so passing a niche is not optional — cut that lane and the path is
//!    severed, which `tests/staging.rs` asserts by actually cutting it.
//!
//! # Anchors, and which way they look
//!
//! * `anchor/niche-<i>` — inside each recess, facing the ledge. The facing is
//!   derived, not declared: the recess scope is reoriented so its local `Z` is
//!   the across-path axis, and a derived facing is the negative direction of
//!   that axis — which points out of the recess at the player. That is why the
//!   ledge is at local `X`-min and the backing at `X`-max, and not the other way
//!   round.
//! * `anchor/niche-watch-<i>` — a ledge cell up-path of the recess, `watch_back`
//!   cells before it, facing down-path. What it is *for* is legibility, and the
//!   fixture proves the legibility rather than asserting it: an unobstructed
//!   sightline to the recess's **mouth** (the ledge cell it opens onto). Not
//!   into the recess — a one-deep recess off a one-wide ledge is geometrically
//!   invisible from anywhere down the path, and that is exactly what makes it an
//!   ambush. The contested ground is the legible thing.
//!
//! **Numbering runs against travel**: `niche-1` is the recess nearest local
//! `Z`-min, i.e. the *last* one the player meets. A split visits its pieces low
//! to high, so declaration order is fixed by the axis, while a derived facing
//! always points the other way down it. With `mark` as it stands the two cannot
//! both follow travel, and a wrong facing is wrong *data* where a numbering
//! convention is only documentation — so the facings win.
//!
//! # Variants
//!
//! Each niche slot is two cells long and draws one of three treatments — the
//! teach / test / twist ladder the souls dossier asks for, as a weighted draw
//! rather than an authoring decision:
//!
//! | Variant | Weight | Shape |
//! |---|---|---|
//! | teach | 2 | one recess with a corpse prop on its floor, no occupant — the tell |
//! | test | 3 | one empty recess, for an occupant the campaign stages |
//! | twist | 1 | two adjacent recesses; the second is the one that gets you |
//!
//! Smallest region that expands: **3 × (`niche_height` + 2) × 3** — three cells
//! across (ledge, recess, backing), and at least as long as it is wide, since
//! the rule turns its length onto the longer horizontal axis. A path shorter
//! than `spacing_min` + 2 is legal and simply has no niches in it.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{
    Alternative, ArithOp, AxisSpec, CmpOp, DimRef, Expr, MarkAt, Node, Program, Reorient,
};

use super::{
    abs, abse, absp, all_of, alt_else, alt_weight, alt_when, at_offset, call, cmp, dim, fill,
    fill_block, int, marked_each, oriented, par, rel, reoriented, split, void,
};

/// How many niche spacings the rule draws between. Spacings run
/// `spacing_min ..= spacing_min + SPACINGS - 1`; the design asks for 6–9.
const SPACINGS: i64 = 4;

/// Cells of a niche slot: the recess, plus the cell the paired variant takes.
const SLOT: i64 = 2;

/// The knockback-niche cliff path.
///
/// Parameters: `spacing_min` (the shortest gap between recesses; the rule draws
/// uniformly over `spacing_min ..= spacing_min + 3`), `niche_height` (how tall a
/// recess is), `watch_back` (how far up-path the watch cell sits). Palette
/// roles: `rock` (the cliff). The teaching variant's corpse prop is not a
/// role: its yaw depends on the scope's orientation, so it is per-orientation
/// guarded inline states (`corpse_prop` below), which one role name cannot
/// express.
///
/// `watch_back` must leave room in the lead — `watch_back + 1 < spacing_min` —
/// or the watch cell falls outside the scope that declares it, which is a loud
/// expansion error naming the anchor.
pub fn cliff_path() -> Program {
    Program::new("cliff_path", "cliff_path")
        .param("spacing_min", 6)
        .param("niche_height", 2)
        .param("watch_back", 3)
        .role("rock", BlockState::simple("stone"))
        // --- frame -----------------------------------------------------------
        // Length runs along whichever horizontal axis the box is longer on, so
        // the rule is reusable turned 90°; up stays up.
        .rule(
            "cliff_path",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("cliff_courses"),
            ),
        )
        // Three courses: floor, the niche band, and everything above it. The
        // guard is the documented minimum region, refused loudly rather than
        // built wrong (there is no `otherwise`).
        .rule_alts(
            "cliff_courses",
            vec![alt_when(
                all_of(vec![
                    cmp(dim(DimRef::X), CmpOp::Ge, int(3)),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("niche_height").arith(ArithOp::Add, int(2)),
                    ),
                ]),
                split(
                    Axis::Y,
                    vec![abs(1), absp("niche_height"), rel(1)],
                    vec![fill("rock"), call("niche_band"), call("wall_lane")],
                ),
            )],
        )
        // A stretch with no recess in it: the ledge open, the wall solid. Also
        // the course above the niches (the lintel) and the floor of the slot
        // cell a single recess does not use — one rule, because they are the
        // same wall.
        .rule(
            "wall_lane",
            split(Axis::X, vec![abs(1), rel(1)], vec![void(), fill("rock")]),
        )
        // --- the niche band ---------------------------------------------------
        .rule_alts("niche_band", niche_band_alts())
        .rule_alts(
            "niche_run",
            vec![
                alt_weight(2, niche_run("recess_teach", "watch_one")),
                alt_weight(3, niche_run("recess_test", "watch_one")),
                alt_weight(1, niche_run("recess_twist", "watch_two")),
            ],
        )
        // A slot is two cells; which of them are cut is the variant.
        .rule(
            "recess_teach",
            split(
                Axis::Z,
                vec![abs(1), abs(1)],
                vec![call("niche_corpse"), call("wall_lane")],
            ),
        )
        .rule(
            "recess_test",
            split(
                Axis::Z,
                vec![abs(1), abs(1)],
                vec![call("niche_empty"), call("wall_lane")],
            ),
        )
        .rule(
            "recess_twist",
            split(
                Axis::Z,
                vec![abs(1), abs(1)],
                vec![call("niche_empty"), call("niche_empty")],
            ),
        )
        .rule("niche_empty", recess_slice(void()))
        .rule(
            "niche_corpse",
            recess_slice(split(
                Axis::Y,
                vec![abs(1), rel(1)],
                vec![call("corpse_prop"), void()],
            )),
        )
        // A skull on the floor of an empty niche: the tell that says "someone
        // stood here and it did not go well". It faces OUT of the recess — the
        // same direction the niche anchor's derived facing points — and a
        // skull's 16-step `rotation` is a literal world yaw that a
        // reorientation does not rewrite, so it cannot be one palette role:
        // it is one alternative per orientation under an `orientation` guard
        // (the `DW0736` mechanism). The recess scope pins local `Y` to world
        // `Y` (the program root does), so these two are the only reachable
        // orientations; a third refuses loudly. Rotation 8 is north, 4 is
        // west — each the negative direction of the world axis the recess
        // calls local `Z`, matching the anchor's derived facing. Before this
        // guard the role carried a literal `rotation=8`, and the same program
        // at a box longer in world X shipped skulls facing along the path
        // instead of out of the niche, silently.
        .rule_alts(
            "corpse_prop",
            vec![
                alt_when(
                    oriented(Axis::X, Axis::Y, Axis::Z),
                    fill_block(BlockState::with("skeleton_skull", [("rotation", "8")])),
                ),
                alt_when(
                    oriented(Axis::Z, Axis::Y, Axis::X),
                    fill_block(BlockState::with("skeleton_skull", [("rotation", "4")])),
                ),
            ],
        )
        // --- watch cells ------------------------------------------------------
        // Declared on the lead, because the lead is the only scope that contains
        // them: a mark may only name a cell of the box its rule was handed.
        .rule(
            "watch_one",
            marked_each("niche-watch", watch_at(0), call("wall_lane")),
        )
        .rule(
            "watch_two",
            marked_each(
                "niche-watch",
                watch_at(0),
                marked_each("niche-watch", watch_at(1), call("wall_lane")),
            ),
        )
}

/// One iteration: a `SLOT`-long niche slot at the low-`Z` end, the lead up-path
/// of it. Pieces run low to high, so the slot is declared first and the watch
/// cells second — which is what keeps `niche-<i>` and `niche-watch-<i>` in step
/// even when the twist variant declares two of each.
fn niche_run(slot: &str, lead: &str) -> Node {
    split(
        Axis::Z,
        vec![abs(SLOT), rel(1)],
        vec![call(slot), call(lead)],
    )
}

/// The spacing draw: one alternative per spacing the box has room for, so a
/// long path draws uniformly over all four and a short one over as many as fit.
/// Overlapping guards are a *distribution*, not a priority order — which is the
/// intent here, and is why they are stated as separate alternatives rather than
/// as one rule with arithmetic.
fn niche_band_alts() -> Vec<Alternative> {
    let mut alts: Vec<Alternative> = (0..SPACINGS)
        .map(|k| {
            let step = par("spacing_min").arith(ArithOp::Add, int(k));
            alt_when(
                // Room for the run itself and a little path beyond it, so the
                // recursion terminates on a plain stretch rather than a stub.
                cmp(
                    dim(DimRef::Z),
                    CmpOp::Ge,
                    step.clone().arith(ArithOp::Add, int(2)),
                ),
                split(
                    Axis::Z,
                    vec![abse(step), rel(1)],
                    vec![call("niche_run"), call("niche_band")],
                ),
            )
        })
        .collect();
    // The tail nearest the approach end: plain path, and where the recursion stops.
    alts.push(alt_else(call("wall_lane")));
    alts
}

/// One cell of path with a recess cut into the inner wall: ledge, recess,
/// backing, and any width beyond that filled.
///
/// Four absolute-then-relative pieces rather than three: the `abs(1)` backing is
/// what makes X = 2 a loud `Overflow` instead of a recess that opens out of the
/// back of the prefab.
fn recess_slice(inner: Node) -> Node {
    split(
        Axis::X,
        vec![abs(1), abs(1), abs(1), rel(1)],
        vec![
            void(),
            // Turn the recess scope so its local Z is the across-path axis: the
            // derived facing is then the negative direction of that axis, i.e.
            // out of the recess at the ledge. This reorientation exists purely
            // to aim the anchor — it moves no block.
            reoriented(
                Reorient::KEEP.z(AxisSpec::LocalX),
                marked_each("niche", MarkAt::CornerMin, inner),
            ),
            fill("rock"),
            fill("rock"),
        ],
    )
}

/// A watch cell: on the ledge lane, on the band's floor, `watch_back` cells
/// up-path of the recess it belongs to (`extra` steps one further for the twist
/// variant's second niche).
fn watch_at(extra: i64) -> MarkAt {
    at_offset(
        int(0),
        int(0),
        par("watch_back")
            .arith(ArithOp::Add, Expr::int(extra))
            .arith(ArithOp::Sub, int(1)),
    )
}
