//! The church — nave, bell tower, windowed side walls, pitched stair roof.
//!
//! Ported rule for rule from `MakeChurch.py` (`yawgmoth/GDMC25`, BSD-3-Clause;
//! author Janista Gitbumrungsin). This is the port that pays for block states
//! being first class: the roof is laid in stairs that must face outward and the
//! door is a two-half door that must face the way the church does. Upstream
//! reads `orientation()` inside the rule body to pick between two pre-registered
//! material ids; here that test is a rule guard ([`Cond::Orientation`]) and the
//! two variants are palette roles, so a caller can restyle the church without
//! knowing which of its blocks are directional.
//!
//! [`Cond::Orientation`]: crate::ir::Cond::Orientation
//!
//! One rule needed a guard upstream does not have: the roof's ridge course is
//! one block wide, and upstream's `rooffill` splits it into two stair edges
//! anyway — which upstream resolves by writing the overshoot outside the region
//! (see `crate::split`). Here the narrow case is its own alternative.
//!
//! Smallest region that expands: the roof steps in 2 blocks of width per block
//! of height, so height must follow width — Y ≥ 9, and Y ≳ X − 3 from about
//! X = 15 up. 15 × 16 × 30 is comfortable; `tests/library.rs` holds these
//! numbers to the code.

use crate::block::BlockState;
use crate::geom::{Axis, Axis::X as WX, Axis::Y as WY, Axis::Z as WZ};
use crate::ir::{AxisSpec, CmpOp, Cond, DimRef, Program, Reorient};

use super::{
    abs, alt_else, alt_when, call, cmp, dim, fill, int, modulo_is, rel, reoriented, split,
    split_repeat, void,
};

/// The church program.
///
/// Parameters: none — the church's proportions come from its guards, which is
/// how upstream wrote it. Palette roles: `wall`, `glass`, `roof_west` /
/// `roof_east` / `roof_north` / `roof_south` (the four stair facings the roof
/// needs), `door_lower` / `door_upper` and `alt_door_lower` / `alt_door_upper`
/// (the door in each of the two orientations the plan can end up in).
pub fn church() -> Program {
    Program::new("church", "church")
        .role("wall", BlockState::simple("stone"))
        .role("glass", BlockState::simple("glass"))
        .role(
            "roof_west",
            BlockState::with(
                "oak_stairs",
                [
                    ("facing", "east"),
                    ("half", "bottom"),
                    ("shape", "straight"),
                    ("waterlogged", "false"),
                ],
            ),
        )
        .role(
            "roof_east",
            BlockState::with(
                "oak_stairs",
                [
                    ("facing", "west"),
                    ("half", "bottom"),
                    ("shape", "straight"),
                    ("waterlogged", "false"),
                ],
            ),
        )
        .role(
            "roof_north",
            BlockState::with(
                "oak_stairs",
                [
                    ("facing", "north"),
                    ("half", "bottom"),
                    ("shape", "straight"),
                    ("waterlogged", "false"),
                ],
            ),
        )
        .role(
            "roof_south",
            BlockState::with(
                "oak_stairs",
                [
                    ("facing", "south"),
                    ("half", "bottom"),
                    ("shape", "straight"),
                    ("waterlogged", "false"),
                ],
            ),
        )
        .role(
            "door_lower",
            BlockState::with(
                "oak_door",
                [
                    ("facing", "north"),
                    ("half", "lower"),
                    ("hinge", "left"),
                    ("open", "false"),
                    ("powered", "false"),
                ],
            ),
        )
        .role(
            "door_upper",
            BlockState::with(
                "oak_door",
                [
                    ("facing", "north"),
                    ("half", "upper"),
                    ("hinge", "left"),
                    ("open", "false"),
                    ("powered", "false"),
                ],
            ),
        )
        .role(
            "alt_door_lower",
            BlockState::with(
                "oak_door",
                [
                    ("facing", "west"),
                    ("half", "lower"),
                    ("hinge", "left"),
                    ("open", "false"),
                    ("powered", "false"),
                ],
            ),
        )
        .role(
            "alt_door_upper",
            BlockState::with(
                "oak_door",
                [
                    ("facing", "west"),
                    ("half", "upper"),
                    ("hinge", "left"),
                    ("open", "false"),
                    ("powered", "false"),
                ],
            ),
        )
        // --- plan --------------------------------------------------------------
        .rule(
            "church",
            reoriented(
                Reorient::KEEP.z(AxisSpec::Largest).y(AxisSpec::LocalY),
                call("layoutroof"),
            ),
        )
        .rule(
            "layoutroof",
            split(
                Axis::Z,
                vec![rel(1), rel(2)],
                vec![call("churchfrontroof"), call("mainhallroof")],
            ),
        )
        // --- front: tower between two blank thirds, then the gable ------------
        .rule("churchfrontroof", call("churchfrontroof1"))
        .rule_alts(
            "churchfrontroof1",
            vec![
                alt_when(
                    modulo_is(dim(DimRef::X), 3, 0),
                    split(
                        Axis::Z,
                        vec![rel(1), abs(1)],
                        vec![call("churchfrontnosides"), call("frontroof")],
                    ),
                ),
                // Not divisible by three: shave a block and try again.
                alt_else(split(
                    Axis::X,
                    vec![rel(1), abs(1)],
                    vec![call("churchfrontroof1"), void()],
                )),
            ],
        )
        .rule(
            "churchfrontnosides",
            split(
                Axis::X,
                vec![rel(1), rel(1), rel(1)],
                vec![void(), call("towerwithroof"), void()],
            ),
        )
        .rule(
            "frontroof",
            split(
                Axis::Y,
                vec![rel(1), rel(2)],
                vec![fill("wall"), call("roofYsplit")],
            ),
        )
        // --- tower --------------------------------------------------------------
        .rule("towerwithroof", call("towerroof1"))
        .rule_alts(
            "towerroof1",
            vec![
                // Trim the tower's footprint square before roofing it.
                alt_when(
                    cmp(dim(DimRef::Z), CmpOp::Gt, dim(DimRef::X)),
                    split(
                        Axis::Z,
                        vec![abs(1), rel(1)],
                        vec![void(), call("towerroof1")],
                    ),
                ),
                alt_else(split(
                    Axis::Y,
                    vec![rel(3), rel(1)],
                    vec![call("tower"), call("roofYsplit")],
                )),
            ],
        )
        .rule("tower", call("tower1"))
        .rule_alts(
            "tower1",
            vec![
                alt_when(
                    cmp(dim(DimRef::Z), CmpOp::Gt, dim(DimRef::X)),
                    split(Axis::Z, vec![abs(1), rel(1)], vec![void(), call("tower1")]),
                ),
                alt_else(split(
                    Axis::Z,
                    vec![abs(1), rel(1), abs(1)],
                    vec![call("towerfront"), call("towerside"), fill("wall")],
                )),
            ],
        )
        .rule(
            "towerfront",
            split(
                Axis::Y,
                vec![abs(2), rel(1)],
                vec![call("towerdoors"), fill("wall")],
            ),
        )
        .rule_alts(
            "towerdoors",
            vec![
                alt_when(
                    modulo_is(dim(DimRef::X), 2, 0),
                    split(
                        Axis::X,
                        vec![rel(1), abs(2), rel(1)],
                        vec![fill("wall"), call("doors"), fill("wall")],
                    ),
                ),
                alt_else(split(
                    Axis::X,
                    vec![rel(1), abs(1), rel(1)],
                    vec![fill("wall"), call("doors"), fill("wall")],
                )),
            ],
        )
        // The door halves, in whichever facing the plan turned the church to.
        .rule_alts(
            "doors",
            vec![
                alt_when(
                    Cond::orientation(WX, WY, WZ),
                    split(
                        Axis::Y,
                        vec![abs(1), abs(1)],
                        vec![fill("door_lower"), fill("door_upper")],
                    ),
                ),
                alt_else(split(
                    Axis::Y,
                    vec![abs(1), abs(1)],
                    vec![fill("alt_door_lower"), fill("alt_door_upper")],
                )),
            ],
        )
        .rule(
            "towerside",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("wall"), void(), fill("wall")],
            ),
        )
        // --- nave ---------------------------------------------------------------
        .rule("mainhallroof", call("mainhallroof1"))
        .rule_alts(
            "mainhallroof1",
            vec![
                alt_when(
                    modulo_is(dim(DimRef::X), 3, 0),
                    split(
                        Axis::Y,
                        vec![rel(1), rel(2)],
                        vec![call("mainhall"), call("roofYsplit")],
                    ),
                ),
                alt_else(split(
                    Axis::X,
                    vec![rel(1), abs(1)],
                    vec![call("mainhallroof1"), void()],
                )),
            ],
        )
        .rule(
            "mainhall",
            split(
                Axis::Z,
                vec![rel(1), abs(1)],
                vec![call("churchsides"), call("back")],
            ),
        )
        .rule("churchsides", call("churchsides1"))
        .rule_alts(
            "churchsides1",
            vec![
                alt_when(
                    modulo_is(dim(DimRef::X), 3, 0),
                    split(
                        Axis::X,
                        vec![abs(1), rel(1), abs(1)],
                        vec![call("windowedwall"), void(), call("windowedwall")],
                    ),
                ),
                alt_else(split(
                    Axis::X,
                    vec![rel(1), abs(1)],
                    vec![call("churchsides1"), void()],
                )),
            ],
        )
        .rule("back", call("back1"))
        .rule_alts(
            "back1",
            vec![
                alt_when(modulo_is(dim(DimRef::X), 3, 0), fill("wall")),
                alt_else(split(
                    Axis::X,
                    vec![rel(1), abs(1)],
                    vec![call("back1"), void()],
                )),
            ],
        )
        // --- windows -------------------------------------------------------------
        .rule_alts(
            "windowedwall",
            vec![
                alt_when(
                    modulo_is(dim(DimRef::Y), 3, 0),
                    split(
                        Axis::Y,
                        vec![rel(1), rel(1), rel(1)],
                        vec![fill("wall"), call("windows"), fill("wall")],
                    ),
                ),
                alt_when(
                    modulo_is(dim(DimRef::Y), 3, 1),
                    split(
                        Axis::Y,
                        vec![abs(1), rel(1), rel(1), rel(1)],
                        vec![fill("wall"), fill("wall"), call("windows"), fill("wall")],
                    ),
                ),
                alt_when(
                    modulo_is(dim(DimRef::Y), 3, 2),
                    split(
                        Axis::Y,
                        vec![abs(1), rel(1), abs(1)],
                        vec![fill("wall"), call("windows"), fill("wall")],
                    ),
                ),
            ],
        )
        .rule_alts(
            "windows",
            vec![
                alt_when(
                    modulo_is(dim(DimRef::Z), 2, 0),
                    split_repeat(
                        Axis::Z,
                        vec![abs(1), abs(1)],
                        vec![fill("wall"), fill("glass")],
                    ),
                ),
                alt_else(split(
                    Axis::Z,
                    vec![rel(1), abs(1)],
                    vec![call("windows"), fill("wall")],
                )),
            ],
        )
        // --- roof ----------------------------------------------------------------
        .rule(
            "roofYsplit",
            split(
                Axis::Y,
                vec![abs(1), rel(1)],
                vec![call("rooffill"), call("roofZsplit")],
            ),
        )
        .rule_alts(
            "roofZsplit",
            vec![
                alt_when(
                    cmp(dim(DimRef::X), CmpOp::Gt, int(2)),
                    split(
                        Axis::X,
                        vec![abs(1), rel(1), abs(1)],
                        vec![void(), call("roofYsplit"), void()],
                    ),
                ),
                alt_else(split(Axis::X, vec![rel(1)], vec![void()])),
            ],
        )
        // One course of the roof: a stair facing out on each edge, solid
        // between. The ridge itself is one block wide and has no room for two
        // edges — upstream splits it anyway and writes the overshoot outside
        // the region (see `split.rs`), so the narrow case is a guard here.
        // The three guards are mutually exclusive, which is what makes this a
        // decision rather than a weighted draw.
        .rule_alts(
            "rooffill",
            vec![
                alt_when(
                    Cond::All {
                        of: vec![
                            Cond::orientation(WX, WY, WZ),
                            cmp(dim(DimRef::X), CmpOp::Ge, int(2)),
                        ],
                    },
                    split(
                        Axis::X,
                        vec![abs(1), rel(1), abs(1)],
                        vec![fill("roof_west"), fill("wall"), fill("roof_east")],
                    ),
                ),
                alt_when(cmp(dim(DimRef::X), CmpOp::Lt, int(2)), fill("wall")),
                alt_else(split(
                    Axis::X,
                    vec![abs(1), rel(1), abs(1)],
                    vec![fill("roof_south"), fill("wall"), fill("roof_north")],
                )),
            ],
        )
        // Kept from upstream but unreferenced by the plan above, exactly as
        // there: the tower with its back wall, and the walled front. They are
        // the alternative front the author left in place.
        .rule(
            "towerroof",
            split(
                Axis::Z,
                vec![rel(1), abs(1)],
                vec![call("towerroof1"), call("towerbackwall")],
            ),
        )
        .rule(
            "towerbackwall",
            split(
                Axis::Y,
                vec![rel(1), rel(1)],
                vec![fill("wall"), call("roofYsplit")],
            ),
        )
        .rule(
            "churchfront",
            split(
                Axis::X,
                vec![rel(1), rel(1), rel(1)],
                vec![
                    call("frontwallsshort"),
                    call("towerroof"),
                    call("frontwallsshort"),
                ],
            ),
        )
        .rule(
            "frontwallsshort",
            split(
                Axis::Y,
                vec![rel(1), rel(1)],
                vec![call("frontwalls"), void()],
            ),
        )
        .rule(
            "frontwalls",
            split(Axis::Z, vec![rel(1), abs(1)], vec![void(), fill("wall")]),
        )
}
