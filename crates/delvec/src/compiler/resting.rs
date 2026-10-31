//! **A block reads as resting on what holds it** (`DW1076`, reported, never
//! refused).
//!
//! A flower pot set on a bottom slab, a lantern on a carpet, a cart bed of two
//! top slabs over the street: each is a block the server keeps, and each shows
//! half a course or more of air under itself, so a player reads it as floating.
//! The judgement is the eye's, made from the pinned jar's own outlines
//! ([`delvewright_dsl::blockshape::outline`], the boxes the game draws):
//!
//! * **the gap** under a block is the air between the bottom of its own outline
//!   and the top of the outline below it, over the block's footprint. A block
//!   is a candidate when the gap is half a course or more ([`GAP_96`]) and it
//!   comes from a half-height surface: the block is one hung in the top half of
//!   its own cell (a top slab, a top trapdoor), or it stands on a block whose
//!   outline stops at half a course or lower (a bottom slab, a carpet). A block
//!   over air is an overhang its neighbours answer for, and is no candidate;
//! * **held otherwise**: a block whose measured `canSurvive` rule
//!   ([`delvewright_dsl::support`]) reads no neighbour below — a hanging lantern,
//!   a wall torch, a ladder — is held where it hangs and is no candidate. A
//!   candidate is held when it, or a candidate it touches across a side face,
//!   touches across a side face a block that is no candidate (a wall, a post, a
//!   beam); or touches, across its top face, a full block that is itself part of
//!   a mass — a full block with a full block beside it (a ceiling, not a bale
//!   set on it);
//! * everything left — a candidate group held by nothing — **reads as floating**
//!   and is reported, with its cell and the piece whose box holds it.
//!
//! The world judged is the load world `DW1002` judges
//! ([`crate::compiler::view::beat::load_blocks`]), its unwritten cells answered
//! as `DW1002` answers them. The report is a warning: whether a block hovers is
//! the author's to see and repair, and the engine states what it measured.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::blockshape::{self, Face, OutlineBox, Sixteenths};
use delvewright_dsl::support::{Needs, needs};
use delvewright_dsl::{Diagnostic, DwCode, ExitTier};

use crate::compiler::attached::Surround;
use crate::compiler::blockstate::BlockMap;
use crate::compiler::plan::Plan;

delvewright_dsl::dw_code! {
    /// `DW1076`: a block the build writes reads as floating — half a course or
    /// more of air stands between its outline and the outline under it, and no
    /// neighbour holds it at its side or from above (`compiler::resting`).
    /// Reported, never refused; the message names each block, its cell and the
    /// piece whose box holds it.
    pub const DW_READS_FLOATING: DwCode = DwCode::new("DW1076", ExitTier::Build);
}

/// A whole course, in 96ths of a block: the outline table's bounds are
/// sixteenths with denominators 1, 2 and 3, and 96 holds every one exactly.
const FULL_96: i64 = 96;

/// The gap that reads as floating: half a course.
pub const GAP_96: i64 = FULL_96 / 2;

/// How many floating blocks the warning names.
const LIST_LIMIT: usize = 12;

const SIDES: [Face; 4] = [Face::North, Face::South, Face::West, Face::East];

fn n96(s: Sixteenths) -> i64 {
    i64::from(s.num) * 6 / i64::from(s.den)
}

/// A box in 96ths: `[x0, y0, z0, x1, y1, z1]`.
type Box96 = [i64; 6];

fn boxes96(name: &str) -> Option<Vec<Box96>> {
    blockshape::outline(name).map(|bs| {
        bs.iter()
            .map(|b: &OutlineBox| {
                [
                    n96(b.min[0]),
                    n96(b.min[1]),
                    n96(b.min[2]),
                    n96(b.max[0]),
                    n96(b.max[1]),
                    n96(b.max[2]),
                ]
            })
            .collect()
    })
}

fn overlap(a0: i64, a1: i64, b0: i64, b1: i64) -> bool {
    a0.max(b0) < a1.min(b1)
}

fn is_full(boxes: &[Box96]) -> bool {
    boxes.contains(&[0, 0, 0, FULL_96, FULL_96, FULL_96])
}

/// Whether `a` (in its cell) and `b` (in the cell across `f`) meet on the
/// shared face with an area between them.
fn touch(a: &[Box96], b: &[Box96], f: Face) -> bool {
    // (axis, a's bound on it, b's bound on it, the two axes the face spans)
    let (axis, hi) = match f {
        Face::West => (0, false),
        Face::East => (0, true),
        Face::Down => (1, false),
        Face::Up => (1, true),
        Face::North => (2, false),
        Face::South => (2, true),
    };
    let (u, v) = match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    a.iter().any(|p| {
        let at_face = if hi {
            p[axis + 3] == FULL_96
        } else {
            p[axis] == 0
        };
        at_face
            && b.iter().any(|q| {
                let meets = if hi {
                    q[axis] == 0
                } else {
                    q[axis + 3] == FULL_96
                };
                meets
                    && overlap(p[u], p[u + 3], q[u], q[u + 3])
                    && overlap(p[v], p[v + 3], q[v], q[v + 3])
            })
    })
}

fn step(c: [i32; 3], f: Face) -> [i32; 3] {
    let o = f.offset();
    [c[0] + o[0], c[1] + o[1], c[2] + o[2]]
}

/// One block that reads as floating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Floating {
    /// Where.
    pub cell: [i32; 3],
    /// The block state written there.
    pub block: String,
    /// The air under it, in 96ths of a block.
    pub gap_96: i64,
    /// What stands under it.
    pub under: String,
    /// The piece whose box holds the cell, if any.
    pub piece: Option<String>,
}

/// What the judgement bound to, and what it found.
#[derive(Debug, Default)]
pub struct Binding {
    /// Non-air cells of the load world that have an outline.
    pub cells: usize,
    /// Of them, blocks with half a course or more of air under them and no
    /// rule holding them elsewhere.
    pub candidates: usize,
    /// Of those, held at a side or from above.
    pub held: usize,
    /// Cells holding a block the outline table does not hold, by block id.
    pub unknown: BTreeMap<String, usize>,
    /// Every block that reads as floating, in cell order.
    pub floating: Vec<Floating>,
}

impl Binding {
    /// The binding line, printed on every build.
    pub fn line(&self) -> String {
        format!(
            "resting blocks (DW1076): {} of {} outlined cell(s) at load stand half a course or \
             more over what is under them — {} held at a side or from above, {} read as floating; \
             not in the outline table: {} cell(s)",
            self.candidates,
            self.cells,
            self.held,
            self.floating.len(),
            self.unknown.values().sum::<usize>(),
        )
    }

    /// The warning, when anything reads as floating: one per piece, so a
    /// place's author reads the cells of the place they are repairing.
    pub fn warnings(&self) -> Vec<Diagnostic> {
        let mut by_piece: BTreeMap<String, Vec<&Floating>> = BTreeMap::new();
        for f in &self.floating {
            by_piece
                .entry(f.piece.clone().unwrap_or_else(|| "(no piece)".into()))
                .or_default()
                .push(f);
        }
        by_piece
            .into_iter()
            .map(|(piece, fs)| {
                let mut msg = format!(
                    "{} block(s) in `{piece}` read as floating: each shows half a course or more \
                     of air between its outline and the outline under it, and nothing beside it or \
                     over it holds it.",
                    fs.len()
                );
                for f in fs.iter().take(LIST_LIMIT) {
                    let [x, y, z] = f.cell;
                    msg.push_str(&format!(
                        "\n  - `{}` at [{x}, {y}, {z}], {}/16 of a course over `{}`",
                        f.block,
                        f.gap_96 / 6,
                        f.under
                    ));
                }
                if fs.len() > LIST_LIMIT {
                    msg.push_str(&format!("\n  … and {} more.", fs.len() - LIST_LIMIT));
                }
                msg.push_str(
                    "\nStand it on a full-height top (a bottom slab one course up, a full block, \
                     a post), set it against what holds it, or choose a block that fills the gap.",
                );
                Diagnostic::warning(DW_READS_FLOATING, "build", piece, msg)
            })
            .collect()
    }
}

/// **Judge the load world** (module header).
pub fn judge(plan: &Plan, load: &BlockMap) -> Binding {
    let surround = Surround::of(plan);
    judge_with(load, &|c| surround.unwritten(c), &|c| {
        surround.inside(c).map(str::to_string)
    })
}

/// [`judge`] over any block map, `unwritten` answering a cell `load` does not
/// hold and `piece` naming the piece whose box holds a cell.
pub fn judge_with(
    load: &BlockMap,
    unwritten: &dyn Fn([i32; 3]) -> &'static str,
    piece: &dyn Fn([i32; 3]) -> Option<String>,
) -> Binding {
    let mut b = Binding::default();
    let name_at =
        |c: [i32; 3]| -> &str { load.get(&c).map(|s| &**s).unwrap_or_else(|| unwritten(c)) };
    // every distinct state's outline, read once
    let mut shapes: BTreeMap<&str, Option<Vec<Box96>>> = BTreeMap::new();
    for state in load.values() {
        let name: &str = state;
        if !shapes.contains_key(name) {
            shapes.insert(name, boxes96(name));
        }
    }
    let shape = |name: &str| -> Option<std::borrow::Cow<'_, [Box96]>> {
        match shapes.get(name) {
            Some(v) => v.as_deref().map(std::borrow::Cow::Borrowed),
            None => boxes96(name).map(std::borrow::Cow::Owned),
        }
    };
    // the candidates, with their gap and what is under them
    let mut cand: BTreeMap<[i32; 3], (i64, String)> = BTreeMap::new();
    for (cell, state) in load {
        let name: &str = state;
        if blockshape::is_air(name) {
            continue;
        }
        let Some(own) = shape(name) else {
            *b.unknown
                .entry(blockshape::base_id(name).to_string())
                .or_default() += 1;
            continue;
        };
        if own.is_empty() {
            continue;
        }
        b.cells += 1;
        if let Some(Needs::Support(rules)) = needs(name)
            && !rules.iter().any(|r| r.cell == Face::Down)
        {
            continue; // it hangs, or is hung on a side: held where it is
        }
        let bottom = own.iter().map(|p| p[1]).min().unwrap_or(0);
        let below_cell = step(*cell, Face::Down);
        let below = name_at(below_cell);
        let under = shape(below).unwrap_or_default();
        let under: &[Box96] = &under;
        let top = under
            .iter()
            .filter(|q| {
                own.iter()
                    .any(|p| overlap(p[0], p[3], q[0], q[3]) && overlap(p[2], p[5], q[2], q[5]))
            })
            .map(|q| q[4])
            .max()
            .unwrap_or(0);
        let gap = bottom + (FULL_96 - top);
        // a surface hung in the top half of its own cell (a top slab, a top
        // trapdoor) whatever is under it; anything else only over a surface
        // that stops at half a course or lower (a bottom slab, a carpet) —
        // over air it is an overhang, which its neighbours answer for
        let half_surface = bottom >= GAP_96 || (!under.is_empty() && top <= GAP_96);
        if half_surface && gap >= GAP_96 {
            cand.insert(*cell, (gap, below.to_string()));
        }
    }
    b.candidates = cand.len();
    // group the candidates by the side faces they touch, and hold a group
    // that touches anything else
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    for start in cand.keys() {
        if seen.contains(start) {
            continue;
        }
        let mut group = vec![*start];
        seen.insert(*start);
        let mut i = 0;
        let mut held = false;
        while i < group.len() {
            let c = group[i];
            i += 1;
            let own = shape(name_at(c)).unwrap_or_default();
            let own: &[Box96] = &own;
            for f in SIDES {
                let n = step(c, f);
                let nn = name_at(n);
                if blockshape::is_air(nn) {
                    continue;
                }
                let theirs = shape(nn).unwrap_or_default();
                if !touch(own, &theirs, f) {
                    continue;
                }
                if cand.contains_key(&n) {
                    if seen.insert(n) {
                        group.push(n);
                    }
                } else {
                    held = true;
                }
            }
            let up = step(c, Face::Up);
            let above = shape(name_at(up)).unwrap_or_default();
            if is_full(&above)
                && touch(own, &above, Face::Up)
                && SIDES
                    .iter()
                    .any(|f| is_full(&shape(name_at(step(up, *f))).unwrap_or_default()))
            {
                held = true;
            }
        }
        if held {
            b.held += group.len();
        } else {
            for c in group {
                let (gap, under) = cand[&c].clone();
                b.floating.push(Floating {
                    cell: c,
                    block: name_at(c).to_string(),
                    gap_96: gap,
                    under,
                    piece: piece(c),
                });
            }
        }
    }
    b.floating.sort_by_key(|f| f.cell);
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::blockstate::BlockState;

    fn judge_map(blocks: &[([i32; 3], &str)]) -> Binding {
        let load: BlockMap = blocks
            .iter()
            .map(|(c, s)| (*c, BlockState::new(s)))
            .collect();
        judge_with(&load, &|_| "minecraft:air", &|_| Some("piece".into()))
    }

    fn ground(extra: &[([i32; 3], &'static str)]) -> Vec<([i32; 3], &'static str)> {
        let mut v: Vec<([i32; 3], &'static str)> = (0..5)
            .flat_map(|x| (0..5).map(move |z| ([x, 0, z], "minecraft:stone")))
            .collect();
        v.extend_from_slice(extra);
        v
    }

    const BOTTOM: &str = "minecraft:stone_brick_slab[type=bottom,waterlogged=false]";
    const TOP: &str = "minecraft:spruce_slab[type=top,waterlogged=false]";

    #[test]
    fn a_pot_on_a_bottom_slab_floats_and_on_a_full_block_rests() {
        let b = judge_map(&ground(&[
            ([1, 1, 1], BOTTOM),
            ([1, 2, 1], "minecraft:potted_white_tulip"),
        ]));
        assert_eq!(b.floating.len(), 1, "{b:?}");
        assert_eq!(b.floating[0].cell, [1, 2, 1]);
        assert_eq!(b.floating[0].gap_96, GAP_96);
        let w = b.warnings();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, "DW1076");
        assert!(
            w[0].message.contains("[1, 2, 1], 8/16 of a course over"),
            "{}",
            w[0].message
        );
        let b = judge_map(&ground(&[
            ([1, 1, 1], "minecraft:stone_bricks"),
            ([1, 2, 1], "minecraft:potted_white_tulip"),
        ]));
        assert!(b.floating.is_empty(), "{b:?}");
        // the bottom slab itself rests on the ground
        let b = judge_map(&ground(&[([1, 1, 1], BOTTOM)]));
        assert!(b.floating.is_empty(), "{b:?}");
    }

    #[test]
    fn a_lone_top_slab_floats_and_one_set_against_a_wall_is_held() {
        // a cart: two top slabs and a bale on them, nothing beside
        let b = judge_map(&ground(&[
            ([2, 1, 1], TOP),
            ([2, 1, 2], TOP),
            ([2, 2, 1], "minecraft:hay_block[axis=z]"),
        ]));
        let cells: Vec<_> = b.floating.iter().map(|f| f.cell).collect();
        assert_eq!(cells, vec![[2, 1, 1], [2, 1, 2]], "{b:?}");
        // the same two slabs run out from a wall: a shelf, held through the group
        let b = judge_map(&ground(&[
            ([2, 1, 0], "minecraft:stone_bricks"),
            ([2, 1, 1], TOP),
            ([2, 1, 2], TOP),
        ]));
        assert!(b.floating.is_empty(), "{b:?}");
        assert_eq!(b.held, 2);
        // a top slab on a fence post still shows half a course of air
        let b = judge_map(&ground(&[
            (
                [3, 1, 3],
                "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=false,west=false]",
            ),
            ([3, 2, 3], TOP),
        ]));
        assert_eq!(b.floating.len(), 1, "{b:?}");
    }

    #[test]
    fn what_hangs_is_held_where_it_hangs_and_a_soffit_under_a_ceiling_is_held() {
        let b = judge_map(&[
            ([1, 4, 1], "minecraft:stone_bricks"),
            (
                [1, 3, 1],
                "minecraft:lantern[hanging=true,waterlogged=false]",
            ),
        ]);
        assert!(
            b.floating.iter().all(|f| f.cell != [1, 3, 1]),
            "a hanging lantern hangs: {b:?}"
        );
        // a top slab under a ceiling course is held from above; the ceiling
        // course is a mass (each block has a full block beside it)
        let b = judge_map(&[
            ([0, 4, 1], "minecraft:stone_bricks"),
            ([1, 4, 1], "minecraft:stone_bricks"),
            ([2, 4, 1], "minecraft:stone_bricks"),
            ([1, 3, 1], TOP),
        ]);
        assert!(b.floating.iter().all(|f| f.cell != [1, 3, 1]), "{b:?}");
        // the binding counts what it judged
        assert!(b.cells >= 4 && b.candidates >= 1, "{b:?}");
    }
}
