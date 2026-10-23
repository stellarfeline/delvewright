//! **What is said about a piece before a person is asked to look at it.**
//!
//! A prefab meets a reviewer's eye through exactly three arms — `delvec viewer`,
//! `delvec render piece` and `delvec render batch` — and each says one thing a
//! person cannot see from a picture before it draws: **how much of the floor no
//! body can reach.** A roofed room with no way into it renders identically to
//! one with a door, and reads as finished. That is `DW0895`, and it is a report
//! rather than a refusal because the engine cannot tell *unreachable* from *not
//! meant to be reached* — a parapet is standable and nobody walks it, a sealed
//! crypt is floor on purpose — and a gate that cannot draw that distinction is
//! one an author learns to route around. What it CAN do is say how much, where,
//! and **which step it was turned back at**.
//!
//! Light is not said here. Light is checked on the whole world and on each
//! region, never on a prefab: whether a surface is walked, and so owed light, is
//! decided by the place a piece is seated in and by where a body can reach.
//!
//! The survey is computed from the piece's own bytes, before textures are
//! resolved and before a GPU is asked for — a report owed to a reviewer is
//! worth nothing after twenty-eight frames have been written.
//!
//! # Nothing here is a second opinion
//!
//! The walk, the standability rule and the roof test are
//! [`crate::schem::nav`]'s; the step rule under them is
//! [`delvewright_dsl::metrics::step_allowed`]; the passability vocabulary is
//! [`delvewright_dsl::blockshape`]. The only thing this module writes for itself
//! is the [`Voxels`] adapter over the renderer's own parsed structure, which is
//! what that trait exists for.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::blockshape;

use crate::compiler::view::diag::Diagnostic;
use crate::compiler::view::nbt::Structure;
use crate::schem::nav::{self, RefusedStep, StepRefusal, Voxels};

delvewright_dsl::dw_code! {
    /// **`DW0895`: floor under a roof that no body can walk to, and the step the
    /// walk was turned back at.** A report, never a refusal — see the module note.
    pub const DW_ENCLOSED_UNREACHED: &str = "DW0895";
}

// ---------------------------------------------------------------------------
// The piece's own bytes, as a box a body meets
// ---------------------------------------------------------------------------

/// The renderer's parsed structure, answering [`Voxels`].
///
/// Cells the template does not mention are air: a vanilla structure template is
/// sparse, and absent means nothing is there. Shared with
/// [`crate::compiler::view::sight`], which stands a room camera by the same rule.
pub struct Cells<'a> {
    size: [i32; 3],
    names: Vec<&'a str>,
}

impl<'a> Cells<'a> {
    /// Index a parsed structure.
    pub fn of(st: &'a Structure) -> Cells<'a> {
        let [sx, sy, sz] = st.size;
        let n = (sx.max(0) as usize) * (sy.max(0) as usize) * (sz.max(0) as usize);
        let mut names: Vec<&str> = vec!["minecraft:air"; n];
        for (pos, state) in &st.blocks {
            if (0..3).all(|a| pos[a] >= 0 && pos[a] < st.size[a])
                && let Some(name) = st.palette.get(*state)
            {
                names[((pos[0] * sy + pos[1]) * sz + pos[2]) as usize] = name.as_str();
            }
        }
        Cells {
            size: st.size,
            names,
        }
    }

    fn name(&self, pos: [i32; 3]) -> Option<&str> {
        if !(0..3).all(|a| pos[a] >= 0 && pos[a] < self.size[a]) {
            return None;
        }
        let [_, sy, sz] = self.size;
        Some(self.names[((pos[0] * sy + pos[1]) * sz + pos[2]) as usize])
    }
}

impl Voxels for Cells<'_> {
    fn origin(&self) -> [i32; 3] {
        [0, 0, 0]
    }

    fn size(&self) -> [i32; 3] {
        self.size
    }

    fn passable(&self, pos: [i32; 3]) -> bool {
        self.name(pos).is_some_and(blockshape::passes_body)
    }

    fn floor(&self, pos: [i32; 3]) -> bool {
        self.name(pos).is_some_and(blockshape::supports_body)
    }

    fn floor_top_16(&self, support: [i32; 3]) -> i64 {
        self.name(support)
            .and_then(blockshape::floor_top_16)
            .map_or(delvewright_dsl::metrics::FULL_16, i64::from)
    }
}

// ---------------------------------------------------------------------------
// `DW0895` — the floor nobody can reach, and the step that cut it off
// ---------------------------------------------------------------------------

/// One lump of floor the walk never entered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pocket {
    /// How many standable cells it holds.
    pub cells: usize,
    /// How many of those are under a roof.
    pub sheltered: usize,
    /// Its box, inclusive.
    pub min: [i32; 3],
    /// Its box, inclusive.
    pub max: [i32; 3],
}

/// The enclosure survey over one piece.
#[derive(Debug, Clone)]
pub struct Enclosure {
    /// Standable cells in the piece — the denominator of everything below.
    pub standable: usize,
    /// Of those, how many are under a roof.
    pub sheltered: usize,
    /// Cells the walk started from: standable, on a vertical face, at grade.
    pub entry_cells: usize,
    /// Standable cells a body can walk to from those.
    pub reached: usize,
    /// Unreached standable cells **under a roof** — floor in a space no body can
    /// get into. The number this diagnostic is about.
    pub stranded_sheltered: usize,
    /// Unreached standable cells open to the sky. Counted, never raised: a roof
    /// is standable and nobody walks it.
    pub stranded_open: usize,
    /// How many disconnected lumps the unreached floor forms.
    pub pockets: usize,
    /// The largest roofed pocket, if any: where to go and look.
    pub largest_roofed: Option<Pocket>,
    /// Steps the walk was refused at the boundary of what it reached.
    pub refused: Vec<RefusedStep>,
}

/// How many refused steps the message names before it stops. One is where an
/// author goes; the totals above it are exact however many there are.
const STEPS_NAMED: usize = 3;

/// **The floor a body reaches from the piece's ground-level entrance**: the
/// standable cells, the entrance ([`nav::ground_entry`]), and the standable
/// cells a body gets to from it by the reach graph's moves over the piece's
/// own blocks ([`crate::compiler::nav::World::floor_reached`] over
/// [`crate::compiler::nav::World::of_piece`]) — a gap a jump clears, a ladder
/// and a ledge stepped off all count, so floor reported unreached is floor no
/// body gets to.
pub(crate) fn floor_reached(st: &Structure, v: &Cells<'_>) -> nav::EnteredFloor {
    let standable = nav::standable_cells(v);
    let entry = nav::ground_entry(v);
    let blocks: BTreeMap<[i32; 3], String> = st
        .blocks
        .iter()
        .filter(|(pos, _)| (0..3).all(|a| pos[a] >= 0 && pos[a] < st.size[a]))
        .filter_map(|(pos, i)| Some((*pos, st.palette.get(*i)?.clone())))
        .collect();
    let reached = crate::compiler::nav::World::of_piece(blocks).floor_reached(&standable, &entry);
    nav::EnteredFloor {
        standable,
        entry,
        reached,
    }
}

/// Run the survey over a piece's own bytes.
pub fn survey(st: &Structure) -> Enclosure {
    let v = Cells::of(st);
    let nav::EnteredFloor {
        standable,
        entry,
        reached,
    } = floor_reached(st, &v);
    let stranded: BTreeSet<[i32; 3]> = standable.difference(&reached).copied().collect();

    let mut stranded_sheltered = 0usize;
    let mut largest: Option<Pocket> = None;
    let components = nav::components(&v, &stranded);
    for component in &components {
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        let mut roofed = 0usize;
        for &cell in component {
            for axis in 0..3 {
                min[axis] = min[axis].min(cell[axis]);
                max[axis] = max[axis].max(cell[axis]);
            }
            if nav::sheltered(&v, cell) {
                roofed += 1;
            }
        }
        stranded_sheltered += roofed;
        let pocket = Pocket {
            cells: component.len(),
            sheltered: roofed,
            min,
            max,
        };
        // Most roofed cells first, then largest, then by position so the choice
        // is total (ADR-0006). Size alone answers the wrong question on a
        // building: the biggest stranded lumps are aisle roofs and tower decks,
        // and the one an author can act on is the room with a ceiling and no
        // door.
        let better = match &largest {
            None => roofed > 0,
            Some(best) => {
                (pocket.sheltered, pocket.cells, best.min)
                    > (best.sheltered, best.cells, pocket.min)
                    && roofed > 0
            }
        };
        if better {
            largest = Some(pocket);
        }
    }

    Enclosure {
        standable: standable.len(),
        sheltered: standable.iter().filter(|&&c| nav::sheltered(&v, c)).count(),
        entry_cells: entry.len(),
        reached: reached.len(),
        stranded_sheltered,
        stranded_open: stranded.len() - stranded_sheltered,
        pockets: components.len(),
        largest_roofed: largest,
        refused: nav::refused_frontier(&v, &standable, &reached),
    }
}

impl Enclosure {
    /// Refused frontier steps that are one course of headroom short.
    pub fn headroom_short(&self) -> Vec<&RefusedStep> {
        self.refused
            .iter()
            .filter(|s| matches!(s.refusal, StepRefusal::Headroom { .. }))
            .collect()
    }

    /// The binding line, printed on every run including a clean one. A zero here
    /// is a measured zero and says so.
    pub fn line(&self, id: &str) -> String {
        format!(
            "enclosure: `{id}` — {} of {} standable cell(s) reachable on foot from {} grade entry \
             cell(s) ({:.1}%); {} under a roof; unreached {} roofed + {} open to the sky, in {} \
             pocket(s); {} refused step(s) at the edge of the walk, {} of them one course of \
             headroom short",
            self.reached,
            self.standable,
            self.entry_cells,
            if self.standable == 0 {
                0.0
            } else {
                self.reached as f64 / self.standable as f64 * 100.0
            },
            self.sheltered,
            self.stranded_sheltered,
            self.stranded_open,
            self.pockets,
            self.refused.len(),
            self.headroom_short().len(),
        )
    }

    /// The report, when roofed floor was stranded.
    ///
    /// Open-to-sky pockets are deliberately not a finding: almost every building
    /// has a roof, a roof is standable, and nobody walks it — raising it every
    /// time is the nag an author learns to skip past, which would cost this
    /// finding its audience.
    pub fn finding(&self, id: &str) -> Option<Diagnostic> {
        if self.stranded_sheltered == 0 {
            return None;
        }
        let where_to_look = self
            .largest_roofed
            .as_ref()
            .map(|p| {
                format!(
                    "the largest is {} cell(s) ({} of them roofed) in the box {},{},{} .. {},{},{}",
                    p.cells,
                    p.sheltered,
                    p.min[0],
                    p.min[1],
                    p.min[2],
                    p.max[0],
                    p.max[1],
                    p.max[2]
                )
            })
            .unwrap_or_else(|| "no pocket of it is roofed".to_string());
        let short = self.headroom_short();
        // **The step, not only the cell.** "This cell is unreachable" is the
        // answer an author can do least with; the boundary step names the
        // opening, and where the refusal is the head sweep it is short by
        // exactly one course, because a standing body owns two cells and a jump
        // needs a third over the TAKEOFF. A flight of stairs whose well is cut
        // one course short reads as walkable in every render, tests fully
        // reachable alone in an open box, and strands every storey above it.
        let cut = if short.is_empty() {
            match self.refused.first() {
                None => String::new(),
                Some(s) => match s.refusal {
                    StepRefusal::Rise { rise_16 } => format!(
                        ". The walk stopped at {},{},{} → {},{},{}, a rise of {:.2} block(s), \
                         which is past the jump apex: no opening fixes that one — the geometry \
                         has to come down or a step has to be built",
                        s.from[0],
                        s.from[1],
                        s.from[2],
                        s.to[0],
                        s.to[1],
                        s.to[2],
                        rise_16 as f64 / delvewright_dsl::metrics::FULL_16 as f64,
                    ),
                    StepRefusal::Headroom { .. } => String::new(),
                },
            }
        } else {
            let named: Vec<String> = short
                .iter()
                .take(STEPS_NAMED)
                .filter_map(|s| match s.refusal {
                    StepRefusal::Headroom { head } => Some(format!(
                        "{},{},{} → {},{},{} needs {},{},{} clear",
                        s.from[0],
                        s.from[1],
                        s.from[2],
                        s.to[0],
                        s.to[1],
                        s.to[2],
                        head[0],
                        head[1],
                        head[2]
                    )),
                    StepRefusal::Rise { .. } => None,
                })
                .collect();
            format!(
                ". {} of the {} step(s) the walk was refused at that edge are a full-block rise \
                 whose TAKEOFF has only two courses over it — the opening is short by exactly one \
                 course, because a standing body owns the cell it is in and the one above, and a \
                 jump needs a third over the takeoff or it head-bonks. Open one course and the \
                 storey connects: {}",
                short.len(),
                self.refused.len(),
                named.join("; ")
            )
        };
        Some(Diagnostic::warning(
            DW_ENCLOSED_UNREACHED,
            format!(
                "`{id}`: {} standable cell(s) UNDER A ROOF have no walking route from the {} grade \
                 entry cell(s) — floor in a space no body can get into, which renders exactly like \
                 a room with a door. {} unreached pocket(s) of floor in all, {} of their cells \
                 open to the sky (a roof, a parapet, a terrace — counted, never raised); {}{}",
                self.stranded_sheltered,
                self.entry_cells,
                self.pockets,
                self.stranded_open,
                where_to_look,
                cut
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The stairwell cut one course short, at the smallest scale that shows
    /// it.** A 7×8×7 hall: ground at `y=0`, a 3×3 dais one block up in the
    /// middle of it, a ceiling at `y=3` over everything BUT the dais, and a roof
    /// at `y=5` over the whole box.
    ///
    /// A body walks the hall floor at `y=1` and the dais top is at `y=2` — one
    /// full block up, which is a jump, which needs the cell two courses over the
    /// TAKEOFF clear. That cell is the `y=3` ceiling. So the dais is roofed
    /// floor a body cannot reach, and `open_the_well` lifts exactly the ring of
    /// ceiling the takeoffs stand under, which is the one course that connects
    /// it. Nothing else moves between the two.
    fn hall_with_a_dais(open_the_well: bool) -> Structure {
        let dais = |x: i32, z: i32| (2..5).contains(&x) && (2..5).contains(&z);
        // The takeoff ring: the hall cells orthogonally beside the dais.
        let beside = |x: i32, z: i32| {
            !dais(x, z)
                && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|(dx, dz)| dais(x + dx, z + dz))
        };
        let mut solid: BTreeSet<[i32; 3]> = BTreeSet::new();
        for x in 0..7 {
            for z in 0..7 {
                solid.insert([x, 0, z]);
                if dais(x, z) {
                    solid.insert([x, 1, z]);
                } else if !(open_the_well && beside(x, z)) {
                    solid.insert([x, 3, z]);
                }
                solid.insert([x, 5, z]);
            }
        }
        Structure {
            size: [7, 8, 7],
            palette: vec!["minecraft:stone".to_string()],
            blocks: solid.into_iter().map(|p| (p, 0usize)).collect(),
        }
    }

    /// The survey names the roofed floor no body reaches, and the step it was
    /// turned back at — with the one cell to open.
    #[test]
    fn roofed_floor_with_no_way_in_is_reported_with_the_step_that_cut_it_off() {
        let e = survey(&hall_with_a_dais(false));
        assert!(
            e.entry_cells > 0,
            "the hall is walked into: {}",
            e.line("d")
        );
        assert_eq!(
            e.stranded_sheltered,
            9,
            "the 3x3 dais is roofed and unreached: {}",
            e.line("dais")
        );
        let d = e.finding("dais").expect("stranded roofed floor reports");
        assert_eq!(d.code, "DW0895");
        assert!(!d.is_error(), "a report, never a refusal");
        assert!(d.message.contains("UNDER A ROOF"), "{}", d.message);
        assert!(
            d.message.contains("short by exactly one course"),
            "{}",
            d.message
        );
        assert!(e.line("dais").contains("one course of headroom short"));
    }

    /// **The perturbation only this check could catch**: open the one course of
    /// ceiling over the takeoffs and the same nine cells become reachable. The
    /// blocks moved are a ring of ceiling that no render distinguishes and no
    /// other gate reads.
    #[test]
    fn opening_the_well_one_course_connects_the_storey() {
        let cut = survey(&hall_with_a_dais(false));
        let open = survey(&hall_with_a_dais(true));
        assert_eq!(cut.stranded_sheltered, 9);
        assert_eq!(open.stranded_sheltered, 0, "{}", open.line("dais"));
        assert_eq!(
            open.reached,
            cut.reached + 9,
            "exactly the dais joins the walk"
        );
        assert!(open.finding("dais").is_none());
        assert!(cut.finding("dais").is_some());
    }

    /// The refusal classifier has exactly two arms and the headroom one names a
    /// cell one course over the takeoff's head — the cell to open, not the cell
    /// that is unreachable.
    #[test]
    fn a_headroom_refusal_names_the_course_to_open() {
        let e = survey(&hall_with_a_dais(false));
        assert!(!e.headroom_short().is_empty(), "{}", e.line("dais"));
        for s in e.headroom_short() {
            match s.refusal {
                StepRefusal::Headroom { head } => {
                    assert_eq!(head, [s.from[0], s.from[1] + 2, s.from[2]]);
                }
                StepRefusal::Rise { .. } => unreachable!("filtered to headroom"),
            }
        }
    }
}
