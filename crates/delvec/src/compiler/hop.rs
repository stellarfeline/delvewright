//! **A hop passes through no block** (spec-0136 §3.1) — `DW1103`, and the
//! binding line that says what it swept.
//!
//! # Why a hop owes a proof
//!
//! A hop is walked by teleport on a body that stays `immovable`
//! (`emit::moves`): nothing collides, so a ceiling lower than the arc's peak
//! does not stop the body — its head goes into the block and comes out again.
//! The player sees a figure sink into a beam; the compiler knows the arc and
//! the room, so the build says it.
//!
//! # What is swept
//!
//! For every `hop` and every body it names, at the cell the body **stands on
//! when the hop fires**: for an NPC whose hop the critical path fires (the
//! flow journal's `fired` names the hop's pointer at one step), the station
//! the cast ledger gives it at that moment — the one question the story
//! pictures ask, [`crate::compiler::view::moment::cast_at`] over
//! [`crate::compiler::view::moment::story_at`], a planned walk's end included —
//! and otherwise the cell it is placed on (`Plan::body_point`, its mark). An
//! NPC the ledger has out of the world at that moment is not swept. There:
//! every cell above the cell its feet stand
//! in, up to the cell the top of its body reaches at the arc's peak
//! ([`delvewright_dsl::hop_arc`]'s greatest height plus the body's own height,
//! `nav::entity_dims`), in every column its width covers. A cell holding a
//! block a body cannot pass ([`delvewright_dsl::blockshape::passes_body`]) is
//! the finding.
//!
//! # What it does not catch
//!
//! A hop the critical path does not fire (a branch's, an ambient trigger's)
//! and an actor's hop are swept at the placed cell alone: a body walked
//! elsewhere for such a hop is not judged there. The body's size is its entity's: a
//! `resize` and a declared `scale` attribute are not followed. The cell the
//! feet stand in is not swept — a slab or a carpet under a body is its floor.

use delvewright_dsl::{BodyKey, DwCode, ExitTier, HOP_UNIT, Verb, hop_arc};

use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;

delvewright_dsl::dw_code! {
    /// `DW1103`: a `hop`'s arc passes the body through a solid block at the
    /// cell it is placed on (spec-0136 §3.1). The message names the hop, the
    /// body, the cell and how high the arc carries the top of the body.
    pub const DW_HOP_THROUGH_A_BLOCK: DwCode = DwCode::new("DW1103", ExitTier::Build);
}

/// **One resolved hop of one body**: what [`judge`] is asked about, once the
/// plan has said where the body stands.
#[derive(Clone, Debug, PartialEq)]
pub struct Hop {
    /// The JSON pointer the effect was found at.
    pub path: String,
    /// The body, as a diagnostic spells it (`npc `npc/keeper``).
    pub body: String,
    /// The cell the body is placed on: its feet stand in it.
    pub cell: [i32; 3],
    /// The body's width, in blocks.
    pub width: f64,
    /// The body's height, in blocks.
    pub height: f64,
    /// The arc's peak above where the body stands, in blocks.
    pub peak: f64,
}

impl Hop {
    /// The highest cell the top of the body enters at the arc's peak.
    pub fn top_cell(&self) -> i32 {
        // A top exactly on a cell boundary enters no cell above it.
        let top = f64::from(self.cell[1]) + self.peak + self.height;
        (top - 1e-9).floor() as i32
    }

    /// Every cell the arc carries the body into above the cell its feet
    /// stand in, column by column, bottom up.
    pub fn swept(&self) -> Vec<[i32; 3]> {
        let half = self.width / 2.0;
        let span = |c: i32| {
            let mid = f64::from(c) + 0.5;
            ((mid - half).floor() as i32)..=((mid + half - 1e-9).floor() as i32)
        };
        let mut out = Vec::new();
        for x in span(self.cell[0]) {
            for z in span(self.cell[2]) {
                for y in (self.cell[1] + 1)..=self.top_cell() {
                    out.push([x, y, z]);
                }
            }
        }
        out
    }
}

/// **The binding of the hop proof**: what it looked at, zeroes included.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HopGate {
    /// `hop` effects declared, at every effect root and any nesting depth.
    pub declared: usize,
    /// `(hop, body)` pairs whose body resolved to a placed cell.
    pub bodies: usize,
    /// Cells swept over every pair.
    pub cells: usize,
    /// Findings raised. The build carries the first; the rest print beside it.
    pub refused: usize,
}

impl HopGate {
    /// The one line this proof owes its reader, printed on every build that
    /// assembles a world and declares a hop.
    pub fn line(&self) -> String {
        format!(
            "hop binding: {} hop(s) declared, {} body arc(s) swept where the story stands the \
             body when the hop fires over {} cell(s), {} refused (DW1103)",
            self.declared, self.bodies, self.cells, self.refused
        )
    }
}

/// Every `hop` the campaign declares, deep, in content order: `(json pointer,
/// height, the bodies it names)`.
fn declared_hops(plan: &Plan) -> Vec<(String, Option<f64>, Vec<BodyKey>)> {
    let mut out = Vec::new();
    fn descend(
        path: String,
        eff: &delvewright_dsl::QuestEffect,
        out: &mut Vec<(String, Option<f64>, Vec<BodyKey>)>,
    ) {
        if let Verb::Hop { height } = &eff.verb {
            out.push((path.clone(), *height, eff.bodies()));
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

/// Whether the campaign declares a hop at all.
pub fn declares_one(plan: &Plan) -> bool {
    !declared_hops(plan).is_empty()
}

/// **Prove every hop passes through no block** (`DW1103`): resolves each
/// hop's bodies against the solved layout and hands them to [`judge`].
pub fn check(
    plan: &Plan,
    world: &crate::compiler::nav::World,
    blocks: &crate::compiler::blockstate::BlockMap,
) -> (HopGate, Vec<Failure>) {
    let declared = declared_hops(plan);
    let mut hops: Vec<Hop> = Vec::new();
    // Where the story stands each NPC when a hop the critical path fires
    // fires: the moment after the step whose journal names the hop.
    let path = crate::compiler::view::beat::PathModel::critical(plan);
    let journal = path.journal(plan);
    let moves = std::sync::OnceLock::new();
    let cast_when = |pointer: &str| -> Option<Vec<crate::compiler::view::moment::CastMark>> {
        let j = journal.iter().position(|j| j.fired.contains(pointer))?;
        let objective = journal[j].objective.as_str();
        let at = path
            .steps
            .iter()
            .position(|s| s.objective() == Some(objective))?;
        let story = crate::compiler::view::moment::story_at(&path.steps, journal, at + 1)?;
        let moves: &Vec<crate::compiler::nav::MovePlan> =
            moves.get_or_init(|| crate::compiler::nav::plan_moves(plan, world).unwrap_or_default());
        Some(crate::compiler::view::moment::cast_at(plan, &story, moves))
    };
    for (path, height, keys) in &declared {
        let peak = hop_arc(*height).into_iter().max().unwrap_or(0);
        let cast = cast_when(path);
        for key in keys {
            // `DW0112` owns a body no stage declares; `DW0325` and its
            // siblings own a body whose mark resolves to nothing.
            let Some(body) = key.resolve(plan.campaign) else {
                continue;
            };
            let dated = match (&key, &cast) {
                (BodyKey::Npc(k), Some(marks)) => {
                    marks.iter().find(|m| m.npc == k.npc.as_str()).map(|m| {
                        m.at.map(|(p, _)| {
                            [
                                p[0].floor() as i32,
                                p[1].floor() as i32,
                                p[2].floor() as i32,
                            ]
                        })
                    })
                }
                _ => None,
            };
            let cell = match dated {
                // The ledger has it out of the world when the hop fires.
                Some(None) => continue,
                Some(Some(c)) => c,
                None => match plan.body_point(body) {
                    Some(c) => c,
                    None => continue,
                },
            };
            let (width, tall) = crate::compiler::nav::entity_dims(body.worn_entity());
            hops.push(Hop {
                path: path.clone(),
                body: format!("{} `{}`", key.class(), key.id()),
                cell,
                width,
                height: tall,
                peak: f64::from(peak) / f64::from(HOP_UNIT),
            });
        }
    }
    let (mut gate, findings) = judge(&hops, blocks);
    gate.declared = declared.len();
    (gate, findings)
}

/// **The rule itself**, over resolved hops — separated from [`check`] so it
/// can be asked directly of a geometry a fixture cannot easily stand up.
pub fn judge(
    hops: &[Hop],
    blocks: &crate::compiler::blockstate::BlockMap,
) -> (HopGate, Vec<Failure>) {
    let mut gate = HopGate::default();
    let mut findings = Vec::new();
    for hop in hops {
        let swept = hop.swept();
        gate.bodies += 1;
        gate.cells += swept.len();
        let hit = swept.iter().find(|c| {
            blocks
                .get(c)
                .is_some_and(|b| !delvewright_dsl::blockshape::passes_body(b))
        });
        if let Some(solid) = hit {
            let clear = solid[1] - hop.cell[1];
            findings.push(Failure {
                code: DW_HOP_THROUGH_A_BLOCK,
                message: format!(
                    "the `hop` at `{path}` carries {body}, placed at {cell:?}, through a block: \
                     the arc peaks {peak} blocks above where it stands, the body is {tall} blocks \
                     tall, so its top reaches the cell at y {top}, and the cell {solid:?} is \
                     solid. A hop is walked by teleport on a body that never collides, so the \
                     ceiling does not stop it — the body's head goes into the block and a player \
                     sees it sink in. There are {clear} cell(s) from its feet to that block. \
                     Lower the hop's `height` so the body's top stays under it, stand the body \
                     where the room is taller, or leave it out of this hop; do NOT remove the \
                     check",
                    path = hop.path,
                    body = hop.body,
                    cell = hop.cell,
                    peak = hop.peak,
                    tall = hop.height,
                    top = hop.top_cell(),
                ),
            });
        }
    }
    gate.refused = findings.len();
    (gate, findings)
}
