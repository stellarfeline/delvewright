//! **A mark stays in the room its anchor names** (`DW0897`, spec-0066 §5.3).
//!
//! A mark is an anchor and an integer block offset from it
//! ([`delvewright_dsl::Mark`]). The offset says *where beside this place*; it
//! never says *which place*. So a mark's cell lies inside the placed piece its
//! anchor belongs to — [`Plan::piece_bounds`], the box wave seating and anchor
//! seating are already confined to. Arithmetic on an offset that reaches past
//! that box stands a body on the horizon or inside the next area, and every
//! geometry proof that assumes a body is in the room its anchor named would be
//! judging the wrong room.
//!
//! The rule quantifies over the three places a campaign puts a body at a mark:
//!
//! * **bodies** — every stage-2 npc and stage-5 actor, through
//!   [`delvewright_dsl::body_sites`] and [`Plan::body_anchor_site`], the one
//!   resolution a body's position has;
//! * **destinations** — every `move-npc`, `move-actor` and `teleport` `to`, at
//!   every depth of every effect root ([`delvewright_dsl::for_each_campaign_effect`]);
//!   a `move-npc` destination resolves in the beat's scope
//!   ([`crate::compiler::plan::BodyScope::Beat`]), exactly as its walk does;
//! * **cast rows** — every placement whose `at` names a mark, resolved in the
//!   beat's scope, exactly as the ledger's per-beat station is.
//!
//! A camera position is a mark too, and is deliberately outside this rule: a
//! dolly flies where the framing wants it, and a shot of a building is taken
//! from outside it. A sound point is outside it for the same reason. A mark
//! whose anchor does not resolve is skipped — `DW0325`/`DW0345`/`DW0360` own
//! dangling references.

use delvewright_dsl::{
    BodyRef, CastPlace, DwCode, EffectSite, ExitTier, Mark, Verb, body_sites,
    for_each_campaign_effect,
};

use crate::compiler::failure::Failure;
use crate::compiler::plan::{BodyScope, Plan, body_station};

/// `DW0897`: **a mark's offset leaves the piece its anchor belongs to.**
///
/// Build tier (exit 3), where the piece boxes are known. The message names the
/// anchor, the offset, the cell it reaches and the box it left; the remedy is
/// the offset.
pub const DW_MARK_LEAVES_PIECE: DwCode = DwCode::new("DW0897", ExitTier::Build);

/// What `DW0897` examined (CLAUDE.md's vacuity rule: every validation artifact
/// states its binding count, with its denominator), printed on every build.
///
/// A campaign that writes no offset prints its zeros and reads as *checked*:
/// `bodies` and `destinations` are the denominators, and `offset_bodies` /
/// `offset_destinations` are how many of them a non-zero offset moved.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MarkBinding {
    /// Body sites the campaign declares, of every class.
    pub bodies: usize,
    /// Of those, the ones standing at a non-zero offset.
    pub offset_bodies: usize,
    /// `move-npc` / `move-actor` / `teleport` destinations, at every depth.
    pub destinations: usize,
    /// Of those, the ones at a non-zero offset.
    pub offset_destinations: usize,
    /// Cast rows whose `at` is spelled as a mark object.
    pub cast_marks: usize,
    /// Marks refused for leaving their anchor's piece (`DW0897`).
    pub refused: usize,
}

impl MarkBinding {
    /// The one line this proof owes its reader (spec-0066 §5.4).
    pub fn line(&self) -> String {
        format!(
            "mark binding: {} body site(s), {} with a non-zero offset; {} destination(s), {} with \
             a non-zero offset; {} cast row(s) at a mark; {} refused for leaving the piece \
             (DW0897).",
            self.bodies,
            self.offset_bodies,
            self.destinations,
            self.offset_destinations,
            self.cast_marks,
            self.refused
        )
    }
}

/// One mark that left its piece, as the refusal names it.
struct Refusal {
    /// What stands there, in words (``actor `actor/hall-page` ``).
    what: String,
    /// The stage and JSON pointer at the declaration.
    stage: &'static str,
    /// JSON pointer at the declaration.
    path: String,
    /// The mark as written.
    mark: Mark,
    /// The area the anchor resolved in.
    area: String,
    /// The anchor's own cell.
    anchor_cell: [i32; 3],
    /// The mark's cell.
    cell: [i32; 3],
    /// The piece box the anchor belongs to.
    bounds: ([i32; 3], [i32; 3]),
}

/// Judge one resolved mark against its anchor's piece box.
#[allow(clippy::too_many_arguments)]
fn judge(
    plan: &Plan<'_>,
    what: String,
    stage: &'static str,
    path: String,
    mark: &Mark,
    area: &str,
    anchor_cell: [i32; 3],
    out: &mut Vec<Refusal>,
) {
    let cell = mark.cell(anchor_cell);
    let (lo, hi) = plan.piece_bounds(area, anchor_cell);
    if (0..3).all(|i| lo[i] <= cell[i] && cell[i] <= hi[i]) {
        return;
    }
    out.push(Refusal {
        what,
        stage,
        path,
        mark: mark.clone(),
        area: area.to_string(),
        anchor_cell,
        cell,
        bounds: (lo, hi),
    });
}

/// **A mark stays in its anchor's piece** (`DW0897`), and what the proof
/// examined. The binding is returned beside the verdict so the caller prints it
/// on every run, the refused run included.
pub fn check_marks_in_piece(plan: &Plan<'_>) -> (MarkBinding, Result<(), Failure>) {
    let c = plan.campaign;
    let mut b = MarkBinding::default();
    let mut refused: Vec<Refusal> = Vec::new();

    // Bodies.
    for s in body_sites(c) {
        b.bodies += 1;
        let mark = s.body.mark();
        if mark.is_offset() {
            b.offset_bodies += 1;
        }
        let Some((area, anchor_cell)) = plan.body_anchor_site(s.body) else {
            continue;
        };
        let what = match s.body {
            BodyRef::Npc(n) => format!("npc `{}`", n.id),
            BodyRef::Actor(a) => format!("actor `{}`", a.id),
        };
        judge(
            plan,
            what,
            s.body.stage(),
            format!("{}/offset", s.path),
            &mark,
            &area,
            anchor_cell,
            &mut refused,
        );
    }

    // Destinations, at every depth of every effect root.
    for_each_campaign_effect(c, &mut |path, site, eff| {
        let (what, to, npc) = match &eff.verb {
            Verb::MoveNpc { npc, to, .. } => {
                (format!("`move-npc` of `{npc}`"), to, Some(npc.as_str()))
            }
            Verb::MoveActor { actor, to, .. } => (format!("`move-actor` of `{actor}`"), to, None),
            Verb::Teleport { to, .. } => ("`teleport`".to_string(), to, None),
            _ => return,
        };
        b.destinations += 1;
        if to.is_offset() {
            b.offset_destinations += 1;
        }
        let resolved = match npc {
            Some(npc) => {
                let home = plan.npc_area(npc).unwrap_or("");
                let beat = match site {
                    EffectSite::Objective { quest, .. } | EffectSite::QuestComplete { quest } => {
                        plan.quest_area(quest)
                    }
                    _ => None,
                }
                .unwrap_or(home);
                body_station(
                    &plan.anchors,
                    BodyScope::Beat { beat, home },
                    to.anchor.as_str(),
                )
                .place()
                .map(|(a, p)| (a.to_string(), p))
            }
            None => plan.point_any_site(to.anchor.as_str()),
        };
        let Some((area, anchor_cell)) = resolved else {
            return;
        };
        judge(
            plan,
            what,
            "quests",
            format!("{path}/to/offset"),
            to,
            &area,
            anchor_cell,
            &mut refused,
        );
    });

    // Cast rows spelled as a mark.
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        let beat_area = plan.quest_area(q.id.as_str()).unwrap_or("");
        for (npc, entry) in &q.cast {
            let placements = entry.placements();
            let many = placements.len() > 1;
            for (pi, p) in placements.into_iter().enumerate() {
                let CastPlace::Mark(mark) = &p.at else {
                    continue;
                };
                b.cast_marks += 1;
                let home = plan.npc_area(npc.as_str()).unwrap_or("");
                let Some((area, anchor_cell)) = body_station(
                    &plan.anchors,
                    BodyScope::Beat {
                        beat: if beat_area.is_empty() {
                            home
                        } else {
                            beat_area
                        },
                        home,
                    },
                    mark.anchor.as_str(),
                )
                .place()
                .map(|(a, p)| (a.to_string(), p)) else {
                    continue;
                };
                let row = if many {
                    format!("/content/quests/{qi}/cast/{npc}/{pi}/at/offset")
                } else {
                    format!("/content/quests/{qi}/cast/{npc}/at/offset")
                };
                judge(
                    plan,
                    format!("quest `{}`'s cast row for `{npc}`", q.id),
                    "quests",
                    row,
                    mark,
                    &area,
                    anchor_cell,
                    &mut refused,
                );
            }
        }
    }

    b.refused = refused.len();
    let Some(first) = refused.first() else {
        return (b, Ok(()));
    };
    let [ox, oy, oz] = first.mark.offset;
    let (lo, hi) = first.bounds;
    let mut message = format!(
        "{} stands at `{}` offset [{ox}, {oy}, {oz}] ({} {}): the anchor resolves to {:?} in \
         area `{}`, and the offset reaches {:?}, outside the box {:?}..={:?} of the placed piece \
         the anchor belongs to. An offset says where beside a place, never which place — a mark \
         that leaves its anchor's piece stands a body somewhere no piece put it, and every proof \
         that reads a body as being in its anchor's room would judge the wrong room. Shorten the \
         offset so the cell stays inside the piece, or name an anchor of the piece the body is \
         meant to stand in.",
        first.what,
        first.mark.anchor,
        first.stage,
        first.path,
        first.anchor_cell,
        first.area,
        first.cell,
        lo,
        hi,
    );
    if refused.len() > 1 {
        let rest: Vec<String> = refused
            .iter()
            .skip(1)
            .map(|r| format!("{} at `{}` ({})", r.what, r.mark.display(), r.path))
            .collect();
        message.push_str(&format!(
            " {} further mark(s) leave their piece the same way: {}.",
            rest.len(),
            rest.join("; ")
        ));
    }
    (
        b,
        Err(Failure {
            code: DW_MARK_LEAVES_PIECE,
            message,
        }),
    )
}
