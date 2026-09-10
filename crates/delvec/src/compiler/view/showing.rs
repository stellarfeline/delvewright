//! **What has to be true of a piece before a person is asked to look at it.**
//!
//! A prefab meets a reviewer's eye through exactly three arms — `delvec viewer`,
//! `delvec render piece` and `delvec render batch` — and until this module ran,
//! all three drew whatever bytes they were handed and said nothing about them.
//! Two things a person cannot see from a picture were therefore never said, and
//! both had already shipped a building:
//!
//! * **nobody had measured the light.** A grammar export declares
//!   `"lighting": {"profile": "unmeasured"}`, which is the *true* statement the
//!   grammar can make — it places blocks, not photons — and the two things in
//!   this engine that measure light are a command somebody has to type
//!   (`delvec prefab lighting`) and the compiler's assembled-world survey, which
//!   quantifies over the areas of a campaign (`DW0210`). A piece that enters no
//!   campaign meets neither. One did: it was exported, rendered to a page, and
//!   walked into by a person who found a room they could not see in, with 1498
//!   of its 8972 walkable cells below light 3 and 984 of them at light 0. That is
//!   `DW0894`, and it is a refusal here because a picture is exactly the medium
//!   in which darkness does not show: the renderer lights every frame for the
//!   camera, not for the body.
//!
//! * **nobody had counted the floor no body can reach.** A roofed room with no
//!   way into it renders identically to one with a door, and reads as finished.
//!   That is `DW0895`, and it is a report rather than a refusal because the
//!   engine cannot tell *unreachable* from *not meant to be reached* — a parapet
//!   is standable and nobody walks it, a sealed crypt is floor on purpose — and a
//!   gate that cannot draw that distinction is one an author learns to route
//!   around. What it CAN do is say how much, where, and **which step it was
//!   turned back at**.
//!
//! # Why the two are one gate and one event
//!
//! Neither is a fact about rendering; both are facts about the piece. What makes
//! this the place to say them is that showing a piece to a person is the moment
//! the piece stops being a file and becomes something somebody is going to
//! believe. Both are computed from the piece's own bytes, before textures are
//! resolved and before a GPU is asked for — a refusal owed to a reviewer is worth
//! nothing after twenty-eight frames have been written.
//!
//! # Nothing here is a second opinion
//!
//! The walk, the standability rule and the roof test are
//! [`crate::schem::nav`]'s, the same ones `delvec prefab lighting` and the
//! grammar back end ask; the step rule under them is
//! [`delvewright_dsl::metrics::step_allowed`]; the passability vocabulary is
//! [`delvewright_dsl::blockshape`]. The only thing this module writes for itself
//! is the [`Voxels`] adapter over the renderer's own parsed structure, which is
//! what that trait exists for.

use std::collections::BTreeSet;

use delvewright_dsl::blockshape;

use crate::compiler::view::diag::Diagnostic;
use crate::compiler::view::meta::PrefabMeta;
use crate::compiler::view::nbt::Structure;
use crate::schem::nav::{self, RefusedStep, StepRefusal, Voxels};

/// **`DW0894`: the piece about to be shown carries no measurement of its own
/// light.** An `unmeasured` profile — or no `lighting` block at all — is refused
/// here, because the alternative is what happened: a person is handed a picture
/// of a building nothing has ever measured, and pictures do not show darkness.
pub const DW_UNMEASURED_LIGHT: &str = "DW0894";

/// **`DW0895`: floor under a roof that no body can walk to, and the step the
/// walk was turned back at.** A report, never a refusal — see the module note.
pub const DW_ENCLOSED_UNREACHED: &str = "DW0895";

// ---------------------------------------------------------------------------
// The piece's own bytes, as a box a body meets
// ---------------------------------------------------------------------------

/// The renderer's parsed structure, answering [`Voxels`].
///
/// Cells the template does not mention are air: a vanilla structure template is
/// sparse, and absent means nothing is there.
struct Cells<'a> {
    size: [i32; 3],
    names: Vec<&'a str>,
}

impl<'a> Cells<'a> {
    fn of(st: &'a Structure) -> Cells<'a> {
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
// `DW0894` — somebody measured the light
// ---------------------------------------------------------------------------

/// What the light half examined and what it found. The counts are stated whether
/// or not anything was wrong: a gate whose binding is not printed cannot be told
/// from one that matched nothing.
#[derive(Debug, Clone)]
pub struct LightVerdict {
    /// Pieces about to be shown — the denominator.
    pub examined: usize,
    /// Of those, how many carry a measured profile.
    pub measured: usize,
    /// Of those, how many carry none and have **nowhere in them to stand**, so
    /// there is no floor for a measurement to be about. Counted and never
    /// refused — and measured off the bytes, never read from the document.
    pub no_player_space: usize,
    /// The ones that do not carry a measurement and do have floor, by piece id,
    /// with what they declare instead.
    pub unmeasured: Vec<(String, &'static str)>,
}

/// Why a piece counts as unmeasured, in the words the refusal prints.
fn unmeasured_reason(meta: Option<&PrefabMeta>) -> Option<&'static str> {
    match meta {
        // No document at all beside the bytes. Not the same claim as an
        // `unmeasured` profile, and the remedy differs, so it says which.
        None => Some("there is no prefab document beside these bytes at all"),
        Some(m) => match &m.lighting {
            None => Some("its prefab document declares no `lighting` block"),
            Some(l) => match l.profile {
                delvewright_dsl::registry::LightingProfile::Unmeasured => {
                    Some("its prefab document declares `\"profile\": \"unmeasured\"`")
                }
                _ => None,
            },
        },
    }
}

impl LightVerdict {
    /// Judge the pieces a run is about to draw, in the order it will draw them.
    ///
    /// `standable` is how many cells of that piece a body can stand in **at
    /// all**, taken from its own bytes by the survey that runs beside this one.
    ///
    /// # The one thing that is not refused, and why it cannot be forged
    ///
    /// A piece with nowhere in it to stand has no floor to be dark: there is
    /// nothing for a measurement to be about, and `unmeasured` is the true
    /// answer rather than a missing one. Five of the rule library's 36 programs
    /// are that — `idiom-arguments`, `idiom-erosion`, `idiom-mirror`,
    /// `idiom-repetition`, `negated-guard`, demonstrations of an IR construct
    /// rather than buildings — and every one of them reports `standable_cells: 0`.
    ///
    /// **This is an escape the defect cannot supply.** It is not a field, a flag
    /// or a word in the document; it is a count the engine takes from the blocks
    /// at the moment of showing. The defect this gate exists to catch is a dark
    /// ROOM, and a room has floor by definition — a piece that could pass through
    /// here has no cell a player could ever be dark in.
    ///
    /// A sealed crypt is deliberately NOT in it: it has floor, so it is refused
    /// until somebody measures it, which is `DW0752`'s "socket it first, then
    /// probe it" and not a reason to show it unmeasured.
    pub fn of<'a>(
        pieces: impl IntoIterator<Item = (&'a str, Option<&'a PrefabMeta>, usize)>,
    ) -> Self {
        let mut v = LightVerdict {
            examined: 0,
            measured: 0,
            no_player_space: 0,
            unmeasured: Vec::new(),
        };
        for (id, meta, standable) in pieces {
            v.examined += 1;
            match unmeasured_reason(meta) {
                None => v.measured += 1,
                Some(_) if standable == 0 => v.no_player_space += 1,
                Some(why) => v.unmeasured.push((id.to_string(), why)),
            }
        }
        v
    }

    /// The binding line, printed on every run including a clean one.
    pub fn line(&self) -> String {
        format!(
            "light measurement: {} of {} piece(s) about to be shown carry a measured lighting \
             profile ({} carr(y/ies) none and have nowhere in them to stand, so there is no floor \
             to measure)",
            self.measured, self.examined, self.no_player_space
        )
    }

    /// The refusal, when anything is unmeasured.
    pub fn finding(&self) -> Option<Diagnostic> {
        if self.unmeasured.is_empty() {
            return None;
        }
        let named: Vec<String> = self
            .unmeasured
            .iter()
            .map(|(id, why)| format!("`{id}` ({why})"))
            .collect();
        Some(Diagnostic::error(
            DW_UNMEASURED_LIGHT,
            format!(
                "{} of {} piece(s) about to be shown have never had their light measured: {}. \
                 Nothing between an unmeasured piece and a person's eye refuses it, and a picture \
                 is the one medium darkness does not show in — the renderer lights every frame \
                 for the camera and not for the body. The grammar's `unmeasured` is honest (it \
                 places blocks, not photons) and the only two things that measure light are this \
                 command's absence of one and the compiler's assembled-world survey (`DW0210`), \
                 which quantifies over the areas of a CAMPAIGN: a piece that enters no campaign \
                 meets neither. A piece the grammar produces is measured where it is produced, so \
                 this is a piece that came from somewhere else, or from an expansion older than \
                 that rule: re-expand it, or measure it over its own bytes with `delvec prefab \
                 lighting <piece> --write`, which floods it at both ends of the sky table, writes \
                 the profile and the binding it was taken over, and reports the DISTRIBUTION of \
                 dark cells rather than the darkest one (`DW0751`). A `dark` result is not a bar \
                 to showing the piece; not knowing is",
                self.unmeasured.len(),
                self.examined,
                named.join(", ")
            ),
        ))
    }

    /// True when this refuses the run.
    pub fn is_refusal(&self) -> bool {
        !self.unmeasured.is_empty()
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

/// Run the survey over a piece's own bytes.
pub fn survey(st: &Structure) -> Enclosure {
    let v = Cells::of(st);
    let standable = nav::standable_cells(&v);
    let entry = nav::ground_entry(&v);
    let reached = nav::reachable_from(&v, &standable, &entry);
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
    use delvewright_dsl::registry::{Lighting, LightingProfile};

    fn lit() -> PrefabMeta {
        PrefabMeta {
            lighting: Some(Lighting {
                profile: LightingProfile::Lit,
                measured_min_light: Some(8),
                measured: Some(String::new()),
                rationale: None,
                method: Some("test".into()),
            }),
            ..PrefabMeta::default()
        }
    }

    fn unmeasured() -> PrefabMeta {
        PrefabMeta {
            lighting: Some(Lighting::unmeasured()),
            ..PrefabMeta::default()
        }
    }

    /// The three ways a piece can be unmeasured are three different sentences,
    /// because they have three different remedies — and all three refuse.
    #[test]
    fn every_shape_of_not_knowing_is_refused_and_says_which() {
        let m = unmeasured();
        let none = PrefabMeta::default();
        // Every one of them has floor: the escape below is a count off the
        // bytes, and a piece with floor cannot reach it.
        let v = LightVerdict::of([
            ("measured", Some(&lit()), 40),
            ("declared-unmeasured", Some(&m), 40),
            ("no-lighting-block", Some(&none), 40),
            ("no-document", None, 40),
        ]);
        assert_eq!((v.examined, v.measured, v.no_player_space), (4, 1, 0));
        let d = v.finding().expect("three unmeasured pieces refuse");
        assert_eq!(d.code, "DW0894");
        assert!(d.is_error());
        assert!(d.message.contains("`declared-unmeasured`"), "{}", d.message);
        assert!(d.message.contains("declares `\"profile\": \"unmeasured\"`"));
        assert!(d.message.contains("declares no `lighting` block"));
        assert!(d.message.contains("no prefab document beside these bytes"));
        assert!(v.line().contains("1 of 4 piece(s)"), "{}", v.line());
    }

    /// A measured `dark` profile is not a bar to showing the piece. The rule is
    /// *somebody measured it*, and the severity of `dark` is `DW0751`'s question,
    /// not this one's — a check that refused `dark` here would be deciding it.
    #[test]
    fn a_measured_dark_piece_is_shown() {
        let mut dark = lit();
        dark.lighting.as_mut().unwrap().profile = LightingProfile::Dark;
        let v = LightVerdict::of([("dark-crypt", Some(&dark), 40)]);
        assert_eq!((v.examined, v.measured), (1, 1));
        assert!(v.finding().is_none());
        assert!(!v.is_refusal());
    }

    /// A run that shows nothing states a zero rather than printing nothing: a
    /// gate whose binding is not written down cannot be told from one that
    /// matched nothing.
    #[test]
    fn a_binding_of_zero_is_stated() {
        let v = LightVerdict::of([]);
        assert!(v.line().contains("0 of 0 piece(s)"), "{}", v.line());
        assert!(v.finding().is_none());
    }

    /// **The one escape, and the thing it demands that the defect cannot
    /// produce.** A piece with nowhere in it to stand has no floor to be dark, so
    /// `unmeasured` is the true answer there and showing it is not a risk. The
    /// escape is a COUNT taken from the bytes at the moment of showing, never a
    /// field, a flag or a word in the document — the perturbation is the one that
    /// matters: give the same undocumented piece a single standable cell and it
    /// is refused.
    #[test]
    fn nowhere_to_stand_is_shown_and_one_cell_of_floor_is_not() {
        let none = PrefabMeta::default();
        let empty = LightVerdict::of([("idiom-mirror", Some(&none), 0)]);
        assert_eq!((empty.examined, empty.no_player_space), (1, 1));
        assert!(empty.finding().is_none(), "nothing to be dark in");
        assert!(!empty.is_refusal());
        assert!(
            empty
                .line()
                .contains("1 carr(y/ies) none and have nowhere in them to stand"),
            "the escape is counted where a reader sees it: {}",
            empty.line()
        );

        let floored = LightVerdict::of([("idiom-mirror", Some(&none), 1)]);
        assert_eq!(floored.no_player_space, 0);
        assert!(
            floored.is_refusal(),
            "one cell a body can stand in is one cell it can be dark in"
        );
        assert_eq!(floored.finding().unwrap().code, "DW0894");
    }

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
