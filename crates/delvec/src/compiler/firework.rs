//! **A firework bursts where the campaign meant it to, and hurts nobody the
//! campaign posted** (spec-0068 §5) — `DW0899`, and the binding ledger that
//! says what it looked at.
//!
//! # Why a firework owes a proof at all
//!
//! A rocket's burst is not decoration: it deals up to
//! [`delvewright_dsl::firework::DAMAGE_ONE_STAR_HP`] HP with one star and two
//! more per star after it, to anything within
//! [`delvewright_dsl::firework::BLAST_RADIUS`] blocks that is not behind a solid
//! block. A verb that can put that beside the guard it was fired over is a verb
//! that owes an answer to *where is the burst*. The emitter makes the answer
//! statable by writing `LifeTime` itself
//! ([`delvewright_dsl::firework::lifetime_ticks`]) instead of letting the game
//! roll it, so the burst stands a fixed [`delvewright_dsl::firework::burst_height`]
//! blocks over the mark and this module can reason about that cell.
//!
//! # Two shapes of one rule
//!
//! * **A roof in the way.** A rocket under a ceiling does not burst where the
//!   page says; it bursts against the ceiling, at a height nothing here can
//!   state and possibly within five blocks of the floor the party stands on. So
//!   the column from the mark to the burst height must be open.
//! * **A posted body in reach.** Any place the campaign requires a body to be
//!   — the entry spawn, every checkpoint and bonfire seat, every NPC and actor
//!   post, every cast placement, every wave seat — within five blocks of the
//!   burst cell. The enumeration is **`DW0511`'s own**
//!   ([`crate::compiler::lethal::posted_places`]), handed over rather than
//!   re-derived, so a post class added for one rule is seen by the other.
//!
//! # What it deliberately does not catch
//!
//! **Players are not posted.** A player standing on a wall walk level with a
//! burst eight blocks over the court, within five blocks of it, takes up to
//! [`delvewright_dsl::firework::worst_damage_hp`] HP — never a full body's
//! twenty, which is why seven stars is the cap — and it is a hazard a player can
//! see coming. No static model can state where a player might stand, so this
//! rule does not pretend to; the demo level is where that is looked at.
//!
//! Reach is judged on **cell distance with no line-of-sight credit**: a wall the
//! page says blocks the damage is not modelled, so the rule is conservative in
//! the safe direction, and it measures the largest axis separation rather than
//! the straight-line one, which makes the refused region a superset of the
//! sphere vanilla damages.

use std::collections::BTreeMap;

use delvewright_dsl::{DwCode, ExitTier, Verb};

use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;

/// `DW0899`: a firework is fired under a roof, or bursts within
/// [`delvewright_dsl::firework::BLAST_RADIUS`] blocks of a place the campaign
/// posts a body (spec-0068 §5).
///
/// One code, because it is one rule — *a firework bursts where the campaign
/// meant it to, and hurts nobody the campaign posted*. What branches is the
/// prescription, and each shape's message names the cell it is about.
pub const DW_FIREWORK_UNSAFE: DwCode = DwCode::new("DW0899", ExitTier::Build);

/// What one firework's proof examined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FireworkRow {
    /// The JSON pointer the effect was found at.
    pub path: String,
    /// The mark the rocket is launched from, as a diagnostic spells it.
    pub mark: String,
    /// The launch cell the mark resolved to.
    pub cell: [i32; 3],
    /// The flight duration in force (the declared one, or the default).
    pub flight: u8,
    /// The column cells checked for a roof — the burst height, in cells.
    pub column: usize,
    /// The cell the burst stands in.
    pub burst: [i32; 3],
}

/// **The binding ledger for the firework proofs** (spec-0068 §5).
///
/// A campaign that declares no firework reports zeroes and says so; a campaign
/// that declares one and resolves none is the unbound shape a reader has to be
/// able to tell from a pass, which is why `declared` and `columns` are separate
/// numbers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FireworkGate {
    /// Fireworks declared, at every effect root and any nesting depth.
    pub declared: usize,
    /// One row per firework whose mark resolved to a cell.
    pub rows: Vec<FireworkRow>,
    /// `(post, burst)` pairs examined for reach.
    pub posts: usize,
    /// Findings raised. The build carries the first; the rest print beside it.
    pub refused: usize,
}

impl FireworkGate {
    /// Column cells examined over every firework.
    pub fn cells(&self) -> usize {
        self.rows.iter().map(|r| r.column).sum()
    }

    /// The ledger as the `validation/firework-gate.json` artifact.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "declared": self.declared,
            "columns": self.rows.len(),
            "column_cells": self.cells(),
            "posts_examined": self.posts,
            "refused": self.refused,
            "fireworks": self.rows.iter().map(|r| serde_json::json!({
                "path": r.path,
                "mark": r.mark,
                "cell": r.cell,
                "flight": r.flight,
                "column": r.column,
                "burst": r.burst,
            })).collect::<Vec<_>>(),
        })
    }

    /// The one line this proof owes its reader, printed on every build that
    /// assembles a world — zeroes included.
    pub fn line(&self) -> String {
        format!(
            "firework binding: {} firework(s) declared, {} burst column(s) checked to {} cell(s), \
             {} post(s) within reach examined, {} refused",
            self.declared,
            self.rows.len(),
            self.cells(),
            self.posts,
            self.refused,
        )
    }
}

/// Every firework the campaign declares, deep, in deterministic content order:
/// `(json pointer, mark, flight, explosions)`.
///
/// The roots come from the single enumeration and the nesting from the single
/// descent authority, so a firework inside a `sequence` step of a trap's payload
/// is asked exactly what a top-level one is.
fn declared_fireworks<'a>(plan: &Plan<'a>) -> Vec<(String, &'a delvewright_dsl::Mark, u8)> {
    let mut out: Vec<(String, &'a delvewright_dsl::Mark, u8)> = Vec::new();
    fn descend<'a>(
        path: String,
        eff: &'a delvewright_dsl::QuestEffect,
        out: &mut Vec<(String, &'a delvewright_dsl::Mark, u8)>,
    ) {
        if let Verb::Firework { at, flight, .. } = &eff.verb {
            out.push((
                path.clone(),
                at,
                flight.unwrap_or(delvewright_dsl::firework::MIN_FLIGHT),
            ));
        }
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            for (j, inner) in list.iter().enumerate() {
                descend(format!("{path}/{pseg}/{j}"), inner, out);
            }
        }
    }
    crate::compiler::plan::for_each_effect_root(plan.campaign, &mut |site, effs| {
        for (i, eff) in effs.iter().enumerate() {
            descend(format!("{}/{i}", site.path), eff, &mut out);
        }
    });
    out
}

/// True if this campaign declares a firework at all — the predicate that decides
/// whether the build owes the proof (and therefore the assembled world it reads).
pub fn declares_one(plan: &Plan) -> bool {
    !declared_fireworks(plan).is_empty()
}

/// Whether a cell's block stops a rocket. Absent from the map = air = open sky.
fn blocks_a_rocket(blocks: &BTreeMap<[i32; 3], String>, cell: [i32; 3]) -> bool {
    blocks
        .get(&cell)
        .is_some_and(|b| !delvewright_dsl::blockshape::passes_body(b))
}

/// **One resolved launch**: what [`judge`] is asked about, once the plan has
/// said where the mark is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    /// The JSON pointer the effect was found at.
    pub path: String,
    /// The mark, as a diagnostic spells it.
    pub mark: String,
    /// The cell the mark resolved to.
    pub cell: [i32; 3],
    /// The flight duration in force.
    pub flight: u8,
}

/// **Prove every firework bursts in open air, clear of every posted body**
/// (`DW0899`).
///
/// Resolves the campaign's fireworks against the solved layout and the posts
/// against `DW0511`'s own enumeration, then hands both to [`judge`], which is
/// where the rule lives.
pub fn check(
    plan: &Plan,
    blocks: &BTreeMap<[i32; 3], String>,
    entry: Option<[i32; 3]>,
    wave_seats: &BTreeMap<String, Vec<[i32; 3]>>,
) -> (FireworkGate, Vec<Failure>) {
    let declared = declared_fireworks(plan);
    if declared.is_empty() {
        return (FireworkGate::default(), Vec::new());
    }
    let mut launches: Vec<Launch> = Vec::new();
    for (path, mark, flight) in &declared {
        // `DW0360` owns a mark whose anchor resolves to nothing.
        if let Some(anchor) = plan.point_any(mark.anchor.as_str()) {
            launches.push(Launch {
                path: path.clone(),
                mark: mark.display(),
                cell: mark.cell(anchor),
                flight: *flight,
            });
        }
    }
    // `DW0511`'s own enumeration, handed over rather than re-derived.
    let posts = crate::compiler::lethal::posted_places(plan, entry, wave_seats);
    let (mut gate, findings) = judge(&launches, &posts, blocks);
    gate.declared = declared.len();
    (gate, findings)
}

/// **The rule itself** (spec-0068 §5), over resolved launches and resolved
/// posts.
///
/// Separated from [`check`] so it can be asked directly of a geometry a
/// campaign fixture cannot easily stand up — a body posted three or more
/// courses above a launch plane, which is the only way a burst eight blocks up
/// can be within five blocks of a post at all.
///
/// Returns the binding beside the findings, so the line a run prints is a count
/// over every firework rather than over the ones that preceded the failure.
/// Findings are in content order; the caller raises the first and prints the
/// rest, as the other assembled-world proofs do. `declared` is the caller's to
/// fill: this function is only told about the launches that resolved.
pub fn judge(
    launches: &[Launch],
    posts: &[crate::compiler::lethal::PostedPlace],
    blocks: &BTreeMap<[i32; 3], String>,
) -> (FireworkGate, Vec<Failure>) {
    use delvewright_dsl::firework;
    let mut gate = FireworkGate {
        declared: launches.len(),
        ..Default::default()
    };
    let mut findings: Vec<Failure> = Vec::new();

    for launch in launches {
        let Launch {
            path,
            mark,
            cell,
            flight,
        } = launch;
        let (cell, flight) = (*cell, *flight);
        let height = firework::burst_height(flight);
        let burst = [cell[0], cell[1] + height, cell[2]];
        let column: Vec<[i32; 3]> = (1..=height)
            .map(|dy| [cell[0], cell[1] + dy, cell[2]])
            .collect();
        gate.rows.push(FireworkRow {
            path: path.clone(),
            mark: mark.clone(),
            cell,
            flight,
            column: column.len(),
            burst,
        });
        gate.posts += posts.len();

        // Shape 1 — a roof in the way. Asked first: a rocket that never reaches
        // its burst height has no burst cell to measure a reach from, so the
        // reach verdict below would be about a place the rocket never gets to.
        if let Some(&solid) = column.iter().find(|&&c| blocks_a_rocket(blocks, c)) {
            findings.push(Failure {
                code: DW_FIREWORK_UNSAFE,
                message: format!(
                    "the `firework` at `{path}`, launched from `{mark}` at {cell:?}, is under a \
                     roof: the cell {solid:?} is solid, and the rocket needs {height} cells of \
                     open air above the mark to burst where flight {flight} puts it. A rocket \
                     under a roof does not burst at a height this engine can state — it bursts \
                     against the ceiling, possibly within {radius} blocks of the floor the party \
                     is standing on, and its burst deals up to {worst} HP. Lower the `flight`, \
                     move the mark under open sky, or build the room taller; do NOT remove the \
                     check.",
                    radius = firework::BLAST_RADIUS,
                    worst = firework::worst_damage_hp(),
                ),
            });
            continue;
        }

        // Shape 2 — a posted body in reach. Cell distance, largest axis, no
        // line-of-sight credit (see the module note).
        let caught: Vec<&crate::compiler::lethal::PostedPlace> = posts
            .iter()
            .filter(|p| {
                (0..3)
                    .map(|i| (p.cell[i] - burst[i]).abs())
                    .max()
                    .is_some_and(|d| d <= firework::BLAST_RADIUS)
            })
            .collect();
        if !caught.is_empty() {
            let named = caught
                .iter()
                .map(|p| format!("{} at {:?}", p.label, p.cell))
                .collect::<Vec<_>>()
                .join("; ");
            findings.push(Failure {
                code: DW_FIREWORK_UNSAFE,
                message: format!(
                    "the `firework` at `{path}`, launched from `{mark}` at {cell:?}, bursts at \
                     {burst:?} — flight {flight} carries it {height} blocks up — and {n} place(s) \
                     the campaign posts a body lie within {radius} blocks of that cell: {named}. \
                     A burst deals up to {worst} HP to a body in reach and no wall is credited \
                     with stopping it, so whatever the campaign puts there is hurt every time the \
                     beat fires. Move the mark, raise the `flight`, or move the post.",
                    n = caught.len(),
                    radius = firework::BLAST_RADIUS,
                    worst = firework::worst_damage_hp(),
                ),
            });
        }
    }
    gate.refused = findings.len();
    (gate, findings)
}
