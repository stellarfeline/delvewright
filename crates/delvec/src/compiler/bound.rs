//! **The boundary and the world agree** (spec-0092 §10) — `DW0960`, and the
//! binding line that says what it looked at.
//!
//! A declared `boundary` derives a region from the placed pieces, and — unless
//! `returns: false` — a per-second clock returns every player outside it to the
//! last checkpoint. Two things can disagree with that region, and each is a
//! defect a player meets and no other proof sees:
//!
//! * **A place the campaign puts a body, outside a region that returns.** A
//!   critical-path step's cell, or the cell a teleport — link or gather — puts a
//!   body on. The party is put there and returned within a second: a carry to a
//!   place outside the region carries nobody, and a step outside it can never be
//!   stood on. The route proof judges walks, footing and carries; it never asked
//!   whether the clock lets a body stay where the route ends.
//! * **A region that does not return, round a world a body can leave.** With
//!   `returns: false` nothing brings a wanderer back, so the switch is legal only
//!   where the build proves no body can walk out of the region or into the open
//!   sea ([`crate::compiler::nav::open_sea_entry`]).
//!
//! Who the clock skips is not judged here: a player watching a cutscene
//! (`dw_cutscene`) and a creator flying with the free camera (`dw_free`) are
//! never returned, so a camera path may leave the region.

use delvewright_dsl::{DwCode, ExitTier};

use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;

delvewright_dsl::dw_code! {
    /// `DW0960`: the declared boundary disagrees with the world — a place the
    /// campaign puts a body lies outside a region that returns, or a region that
    /// does not return encloses a world a body can leave (spec-0092 §10).
    pub const DW_BOUND_DISAGREES: DwCode = DwCode::new("DW0960", ExitTier::Build);
}

/// What the proof examined.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BoundGate {
    /// Whether a boundary is declared at all.
    pub declared: bool,
    /// Whether it returns.
    pub returns: bool,
    /// Places a body is put, examined against the region.
    pub places: usize,
    /// Of those, outside it.
    pub outside: usize,
    /// Reachable walkable cells examined (only for a region that does not return).
    pub reached: usize,
    /// Findings raised.
    pub refused: usize,
}

impl BoundGate {
    /// The one line every build that assembles a world prints.
    pub fn line(&self) -> String {
        if !self.declared {
            return "boundary binding: no boundary declared — 0 place(s) examined, 0 refused"
                .to_string();
        }
        format!(
            "boundary binding: the region {}; {} place(s) a body is put examined, {} outside; \
             {} reachable cell(s) examined for a way out; {} refused",
            if self.returns {
                "returns"
            } else {
                "does not return"
            },
            self.places,
            self.outside,
            self.reached,
            self.refused,
        )
    }
}

fn inside(c: [i32; 3], (lo, hi): ([i32; 3], [i32; 3])) -> bool {
    (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i])
}

/// Every place the campaign puts a body, labelled: each critical-path step's
/// cell, and the cell each link and gather puts a body on.
pub fn places(plan: &Plan) -> Vec<(String, [i32; 3])> {
    let mut out: Vec<(String, [i32; 3])> = Vec::new();
    for (i, step) in plan.critical_path.iter().enumerate() {
        if let Some(p) = step.pos() {
            out.push((format!("critical-path step {i}"), p));
        }
    }
    for l in &plan.links {
        out.push((
            format!("the link `{}` (`{}`) lands", l.trigger_id, l.path),
            l.to,
        ));
    }
    for g in &plan.gathers {
        out.push((format!("the teleport `{}` lands", g.path), g.to));
    }
    out
}

/// **Prove the boundary and the world agree** (`DW0960`). `region` is the
/// derived playable region (`None`: no boundary); `reachable` is the walk
/// region and `sea_entry` the first cell a body walks into the open sea from,
/// both read only for a region that does not return.
pub fn judge(
    places: &[(String, [i32; 3])],
    region: Option<([i32; 3], [i32; 3])>,
    returns: bool,
    reachable: &std::collections::BTreeSet<[i32; 3]>,
    sea_entry: Option<[i32; 3]>,
) -> (BoundGate, Vec<Failure>) {
    let Some(region) = region else {
        return (BoundGate::default(), Vec::new());
    };
    let mut gate = BoundGate {
        declared: true,
        returns,
        places: places.len(),
        ..Default::default()
    };
    let mut findings = Vec::new();
    let (lo, hi) = region;
    if returns {
        let out: Vec<&(String, [i32; 3])> =
            places.iter().filter(|(_, c)| !inside(*c, region)).collect();
        gate.outside = out.len();
        if !out.is_empty() {
            let named = out
                .iter()
                .take(6)
                .map(|(what, c)| format!("{what} at {c:?}"))
                .collect::<Vec<_>>()
                .join("; ");
            findings.push(Failure {
                code: DW_BOUND_DISAGREES,
                message: format!(
                    "{n} place(s) the campaign puts a body lie outside the boundary's region \
                     {lo:?}..{hi:?}: {named}. The boundary returns every player outside it to \
                     the last checkpoint once a second, so a body put there is taken back before \
                     it can stand there — a carry there carries nobody, and a step there can \
                     never be stood on. The region is the placed pieces grown by `margin`: \
                     place a piece (a site-plan box, an area) under the destination, raise \
                     `margin`, or move the destination into the region.",
                    n = out.len(),
                ),
            });
        }
    } else {
        gate.reached = reachable.len();
        let walked_out = reachable.iter().find(|c| !inside(**c, region)).copied();
        if let Some(c) = sea_entry.or(walked_out) {
            let how = if sea_entry.is_some() {
                format!("a body walks into the open sea from {c:?}")
            } else {
                format!("a body walks to {c:?}, outside it")
            };
            findings.push(Failure {
                code: DW_BOUND_DISAGREES,
                message: format!(
                    "the boundary declares `returns: false`, so nothing brings a player back \
                     from outside its region {lo:?}..{hi:?} — and {how}. The switch is for a \
                     world nobody can leave. Seal the edge (a rail, a wall, a sea gate) so the \
                     walk cannot get out, or let the boundary return."
                ),
            });
        }
    }
    gate.refused = findings.len();
    (gate, findings)
}
