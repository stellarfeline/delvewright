//! **A harmless body says what it is** (spec-0119): the answer a staged body
//! gives a player who strikes or presses it, the box that carries the answer,
//! and the refusal of a monster-kind statue in reach that says nothing.
//!
//! ## The class
//!
//! A *body* is a stage-2 NPC or a stage-5 actor ([`BodyRef`]). Each may declare
//! [`delvewright_dsl::BodyAnswers`]: one `Refusal` line per press — `struck` for
//! a blow, `touched` for a right-click (an actor's only; an NPC's right-click is
//! its dialogue, `DW1056`). Each declared line is lowered into the press-answer
//! path every compiler-synthesized answer takes ([`BodyAnswer::trigger`]): an
//! ordinary `audience: presser` [`EnvTrigger`] carrying one `narrate` on the
//! action bar, dispatched as the player who pressed. Nothing here is a second
//! emitter: the trigger rides a hitbox the body already owns (an NPC's dialogue
//! hitbox) or the answer box the body gains ([`ANSWER_BOX_TAG`]), and is emitted,
//! advanced, dispatched and PackTested by the code that emits every trigger.
//!
//! ## Who names the striker
//!
//! A left-click on a `minecraft:interaction` fires
//! `minecraft:player_hurt_entity` with the interaction as `this`, granted to the
//! attacking player — measured on the pinned jar
//! (`docs/reference/click-attribution.md`, its instrument
//! `tools/maintenance/probe-click-attribution.py`). A left-click on an
//! `Invulnerable` living body fires nothing. So an inert body answers a blow
//! only through an interaction box that encloses it, and the box is what this
//! module sizes ([`answer_box_dims`]).
//!
//! ## What is owed (`DW1055`)
//!
//! The ruling binds four facts the tree holds (spec-0119 §2): the body's
//! shipping kind is one the pinned game discards on peaceful
//! ([`delvewright_dsl::removed_on_peaceful`], the game's own statement of which
//! kinds are monsters); the body is inert ([`BodyRef::inert`]); no `strike` /
//! `strike-npc` trigger rides its blow ([`BodyRef::struck_rider`]); and at some
//! station it is declared to stand on — its home mark, every `move-npc` /
//! `move-actor` destination — a walking player can act on it
//! ([`crate::compiler::nav::interaction_stances`], the one question `DW1037`
//! asks of a holder). Such a body owes `struck`, and an actor `touched` too.

use std::collections::BTreeSet;

use delvewright_dsl::{
    BodyPress, BodyRef, Campaign, DwCode, EnvTrigger, ExitTier, Mark, QuestEffect, Verb,
};

use crate::compiler::failure::Failure;
use crate::compiler::plan::{Plan, safe_local};

delvewright_dsl::dw_code! {
    /// `DW1055` (spec-0119): **a harmless monster-kind body in reach with no
    /// line.** A body whose shipping kind the pinned game discards on peaceful,
    /// that cannot be harmed and does not fight, that no `strike` / `strike-npc`
    /// trigger rides, and that a walking player can reach at a station it is
    /// declared to stand on, declares no `answers.struck` (or, an actor, no
    /// `answers.touched`). A player who strikes it meets a blow that does
    /// nothing and says nothing, and cannot tell a statue from a broken fight.
    /// The engine never writes the words.
    pub const DW_BODY_SILENT: DwCode = DwCode::new("DW1055", ExitTier::Build);
}

/// The tag every actor's **answer box** wears, suffixed with the actor's safe
/// id: the `minecraft:interaction` that encloses an inert puppet declaring
/// `answers`, so a blow and a touch on the puppet land on a box vanilla names the
/// presser for.
pub const ANSWER_BOX_TAG: &str = "dw_abody_";

/// The answer box's tag for `actor` (`dw_abody_<safe>`).
pub fn answer_box_tag(actor: &str) -> String {
    format!("{ANSWER_BOX_TAG}{}", safe_local(actor))
}

/// **The enclosing rule**: the `(width, height)` of the interaction box a body
/// of shipping kind `entity` wears — its standing hitbox
/// ([`crate::compiler::nav::entity_dims`]) grown by
/// [`crate::compiler::clickbox::CLICK_MARGIN`] past every face a ray meets
/// first (both sides, and the top), never smaller than the `1.0 × 2.0` box every
/// NPC has always worn.
///
/// The ray picks the first box it meets (`crosshair.rs`), and a body larger
/// than its box is picked instead of it (`DW0359`'s giant), so a box that does
/// not enclose the body loses the blow to the body — and a blow on an
/// `Invulnerable` body names nobody. The `1.0 × 2.0` floor is the instance of
/// this rule for every body up to that size, so a villager's or a zombie's box
/// is the box it always was. Computed in hundredths so the emitted figure is
/// exactly what the rule reads (ADR-0006).
pub fn answer_box_dims(entity: &str) -> (f64, f64) {
    let (w, h) = crate::compiler::nav::entity_dims(entity);
    let m = crate::compiler::clickbox::CLICK_MARGIN_CENTI;
    let centi = |v: f64| (v * 100.0).round() as i64;
    let width = (centi(w) + 2 * m).max(centi(crate::compiler::emit::NPC_HITBOX_WIDTH));
    let height = (centi(h) + m).max(centi(crate::compiler::emit::NPC_HITBOX_HEIGHT));
    (width as f64 / 100.0, height as f64 / 100.0)
}

/// A box dimension as the summon NBT spells it: one decimal where one is
/// enough (`1.0`, `2.0` — every box the constant ever wrote), two otherwise.
pub fn dim_snbt(v: f64) -> String {
    let c = (v * 100.0).round() as i64;
    if c % 10 == 0 {
        format!("{:.1}", v)
    } else {
        format!("{:.2}", v)
    }
}

/// One declared answer: the body, the press, and the line.
#[derive(Clone, Debug)]
pub struct BodyAnswer<'c> {
    /// The body that answers.
    pub body: BodyRef<'c>,
    /// Which press it answers.
    pub press: BodyPress,
    /// The full id of the trigger this lowers to
    /// (`trigger/dw-answer-<class>-<local>-<press>`).
    pub trigger_id: String,
    /// The campaign's own line, l10n-tagged with its key.
    pub text: String,
}

/// The trigger id a body answer is synthesized under. `dw-` is reserved from
/// authored ids (`DW0428`); `<class>` (`npc` / `actor`) separates an NPC and an
/// actor that share a local name, which would otherwise land on one id.
fn answer_trigger_id(body: BodyRef<'_>, press: BodyPress) -> String {
    let class = match body {
        BodyRef::Npc(_) => "npc",
        BodyRef::Actor(_) => "actor",
    };
    let local = body.id().split_once('/').map_or(body.id(), |(_, r)| r);
    format!("trigger/dw-answer-{class}-{local}-{}", press.token())
}

/// **Every declared body answer**, in body order (stage-2 NPCs, then stage-5
/// actors, each in declaration order), presses in [`BodyPress::ALL`] order.
/// Deterministic: two fixed orders, no hashing (ADR-0006).
///
/// A line `DW1056` would refuse never reaches here: validation ends the run
/// first.
pub fn answers(c: &Campaign) -> Vec<BodyAnswer<'_>> {
    let mut out = Vec::new();
    for site in delvewright_dsl::body_sites(c) {
        let Some(a) = site.body.answers() else {
            continue;
        };
        for press in site.body.presses() {
            if let Some(text) = a.line(*press) {
                out.push(BodyAnswer {
                    body: site.body,
                    press: *press,
                    trigger_id: answer_trigger_id(site.body, *press),
                    text: text.clone(),
                });
            }
        }
    }
    out
}

/// The body answer a synthesized trigger id belongs to, if it is one.
pub fn answer_of<'c>(c: &'c Campaign, trigger_id: &str) -> Option<BodyAnswer<'c>> {
    answers(c).into_iter().find(|a| a.trigger_id == trigger_id)
}

/// The `dw_trig_<id>` tags of every answer an actor's box carries, in press
/// order. Empty for an actor that declares no `answers`, which therefore wears
/// no box.
pub fn actor_box_trigger_tags(c: &Campaign, actor: &str) -> Vec<String> {
    answers(c)
        .into_iter()
        .filter(|a| matches!(a.body, BodyRef::Actor(x) if x.id.as_str() == actor))
        .map(|a| format!("dw_trig_{}", safe_local(&a.trigger_id)))
        .collect()
}

/// The `dw_trig_<id>` tag of an NPC's `struck` answer, if it declares one —
/// worn by the NPC's own dialogue hitbox, its carrier.
pub fn npc_hitbox_answer_tags(c: &Campaign, npc: &str) -> Vec<String> {
    answers(c)
        .into_iter()
        .filter(|a| matches!(a.body, BodyRef::Npc(x) if x.id.as_str() == npc))
        .map(|a| format!("dw_trig_{}", safe_local(&a.trigger_id)))
        .collect()
}

impl BodyAnswer<'_> {
    /// The answer lowered into the general verb: a repeatable press with no cell
    /// of its own — it rides its body's box — that puts the campaign's line on
    /// the presser's action bar.
    ///
    /// The event is the press kind's own: a blow on an NPC is `strike-npc` (the
    /// body's dialogue hitbox is its carrier, as for every `strike-npc`); a blow
    /// on an actor is `strike`, a touch `use`, each carried by the actor's
    /// answer box. Ungated: the box stands exactly while the body is inert, so
    /// the answer's state is the box's existence.
    pub fn trigger(&self) -> EnvTrigger {
        let on = match (self.body, self.press) {
            (BodyRef::Npc(n), _) => delvewright_dsl::TriggerOn::StrikeNpc { npc: n.id.clone() },
            (BodyRef::Actor(_), BodyPress::Struck) => delvewright_dsl::TriggerOn::Strike,
            (BodyRef::Actor(_), BodyPress::Touched) => delvewright_dsl::TriggerOn::Use,
        };
        EnvTrigger {
            id: delvewright_dsl::TriggerId(self.trigger_id.clone()),
            at: None,
            prop: None,
            on,
            requires_flags: Vec::new(),
            forbids_flags: Vec::new(),
            requires_state: Vec::new(),
            once: false,
            audience: delvewright_dsl::TriggerAudience::Presser,
            effects: vec![
                Verb::Narrate(delvewright_dsl::Narration {
                    text: self.text.clone(),
                    style: Some(delvewright_dsl::NarrateStyle::Actionbar),
                    sound: None,
                })
                .into(),
            ],
        }
    }

    /// The tag of the box this answer rides, and what owns it.
    pub fn carrier(&self, plan: &Plan) -> (String, &'static str) {
        match self.body {
            BodyRef::Npc(n) => (
                plan.npcs
                    .iter()
                    .find(|p| p.npc_id == n.id.as_str())
                    .map_or_else(
                        || format!("dw_npc_{}", safe_local(n.id.as_str())),
                        |p| p.tag.clone(),
                    ),
                "NPC's dialogue hitbox",
            ),
            BodyRef::Actor(a) => (answer_box_tag(a.id.as_str()), "actor's answer box"),
        }
    }
}

// ---------------------------------------------------------------------------
// DW1055 — who owes a line
// ---------------------------------------------------------------------------

/// What `DW1055` found, over every body the campaign declares.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnswerBinding {
    /// `B`: every body declared (the denominator).
    pub bodies: usize,
    /// `M`: bodies whose shipping kind the game discards on peaceful.
    pub monster: usize,
    /// `I`: of those, the inert ones.
    pub inert: usize,
    /// `T`: of those, the ones a `strike` / `strike-npc` trigger rides.
    pub carried: usize,
    /// `R`: inert monster-kind bodies no trigger rides, reachable at a station.
    pub in_reach: usize,
    /// `S`: inert monster-kind bodies no trigger rides, reachable at no station.
    pub out_of_reach: usize,
    /// `O`: lines owed.
    pub owed: usize,
    /// `D`: lines declared, on every body.
    pub declared: usize,
    /// `V`: lines declared on bodies that owe none.
    pub volunteered: usize,
    /// `X`: bodies refused.
    pub refused: usize,
    /// One row per body, in body order — the record `validation/body-answers.json`
    /// carries.
    pub rows: Vec<AnswerRow>,
}

/// What `DW1055` read for one body.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnswerRow {
    /// The body's id.
    pub id: String,
    /// The kind it ships as.
    pub kind: String,
    /// `M`: the game discards that kind on peaceful.
    pub monster: bool,
    /// `I`: it cannot be harmed and does not fight while staged.
    pub inert: bool,
    /// `T`: the authored trigger that rides its blow, if any.
    pub carried_by: Option<String>,
    /// The station that reaches it and the stance cell, when one does.
    pub reached: Option<([i32; 3], [i32; 3])>,
    /// The presses it owes a line for.
    pub owed: Vec<&'static str>,
    /// The presses it declares a line for.
    pub declared: Vec<&'static str>,
}

impl AnswerBinding {
    /// The line every build prints, zeroes included.
    pub fn line(&self) -> String {
        format!(
            "body answers (DW1055): {} bodies — {} monster-kind, {} inert, {} in reach, {} lines \
             owed, {} declared, {} volunteered on kinds not owed, {} refused; {} out of reach, {} \
             carried by a strike trigger",
            self.bodies,
            self.monster,
            self.inert,
            self.in_reach,
            self.owed,
            self.declared,
            self.volunteered,
            self.refused,
            self.out_of_reach,
            self.carried
        )
    }

    /// The record as `validation/body-answers.json`: the binding, with every
    /// column and its denominator, and one row per body.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": DW_BODY_SILENT,
            "binding": {
                "bodies": self.bodies,
                "monster_kind": self.monster,
                "inert": self.inert,
                "in_reach": self.in_reach,
                "lines_owed": self.owed,
                "lines_declared": self.declared,
                "volunteered": self.volunteered,
                "refused": self.refused,
                "out_of_reach": self.out_of_reach,
                "carried_by_strike_trigger": self.carried,
            },
            "bodies": self.rows.iter().map(|r| serde_json::json!({
                "id": r.id,
                "kind": r.kind,
                "monster_kind": r.monster,
                "inert": r.inert,
                "carried_by": r.carried_by,
                "station": r.reached.map(|(s, _)| s),
                "stance": r.reached.map(|(_, c)| c),
                "owed": r.owed,
                "declared": r.declared,
            })).collect::<Vec<_>>(),
        })
    }
}

/// Every station a body is declared to stand on (spec-0119 §2): its home mark,
/// then every `move-npc` / `move-actor` destination, in campaign effect order,
/// each once. A station whose anchor resolves nowhere is skipped: placement
/// refuses it elsewhere (`DW0325`, `DW0307`).
pub fn stations(plan: &Plan, body: BodyRef<'_>) -> Vec<([i32; 3], String)> {
    let mut out: Vec<([i32; 3], String)> = Vec::new();
    let home = match body {
        BodyRef::Npc(n) => Some(crate::compiler::watching::npc_home(plan, n).0),
        BodyRef::Actor(_) => plan.body_point(body),
    };
    if let Some(h) = home {
        out.push((
            h,
            format!("its home mark `{}`", body.mark().anchor.as_str()),
        ));
    }
    let mut dests: Vec<Mark> = Vec::new();
    delvewright_dsl::for_each_campaign_effect(plan.campaign, &mut |_, _, e: &QuestEffect| {
        let to = match (&e.verb, body) {
            (Verb::MoveNpc { npc, to, .. }, BodyRef::Npc(n)) if *npc == n.id => to,
            (Verb::MoveActor { actor, to, .. }, BodyRef::Actor(a)) if *actor == a.id => to,
            _ => return,
        };
        if !dests.contains(to) {
            dests.push(to.clone());
        }
    });
    for to in dests {
        if let Some(cell) = plan.point_any(to.anchor.as_str()).map(|p| to.cell(p))
            && !out.iter().any(|(c, _)| *c == cell)
        {
            out.push((cell, format!("the walk to `{}`", to.anchor.as_str())));
        }
    }
    out
}

/// The standing box of a body of shipping kind `entity` on `cell`: its hitbox
/// centred on the cell's centre, rising from the cell floor — as `eclipse.rs`
/// sizes a body.
pub fn body_box(entity: &str, cell: [i32; 3]) -> crate::compiler::nav::BodyBox {
    let (w, h) = crate::compiler::nav::entity_dims(entity);
    let [x, y, z] = cell.map(f64::from);
    crate::compiler::nav::BodyBox {
        lo: [x + 0.5 - w / 2.0, y, z + 0.5 - w / 2.0],
        hi: [x + 0.5 + w / 2.0, y + h, z + 0.5 + w / 2.0],
    }
}

/// **Where a player can stand to strike or press `bx`**: the one stance
/// question ([`crate::compiler::nav::interaction_stances`]), less the cells the
/// body itself fills — a player does not stand inside the statue to hit it, as a
/// holder's stance excludes the cells holder entities fill.
pub fn reach_cells(
    world: &crate::compiler::nav::World,
    bx: &crate::compiler::nav::BodyBox,
    reachable: &BTreeSet<[i32; 3]>,
) -> Vec<[i32; 3]> {
    let filled = |c: [i32; 3]| {
        (0..3).all(|i| {
            let lo = f64::from(c[i]);
            lo < bx.hi[i] && lo + 1.0 > bx.lo[i]
        })
    };
    crate::compiler::nav::interaction_stances(world, bx, reachable)
        .into_iter()
        .filter(|c| !filled(*c))
        .collect()
}

/// One body's reading.
struct Reading<'c> {
    body: BodyRef<'c>,
    path: String,
    owed: Vec<BodyPress>,
    reach: Option<(String, [i32; 3], [i32; 3])>,
}

/// **`DW1055`**: every body owed a line declares it. Returns the binding (always)
/// and the verdict.
///
/// The walk-reachable set is `world.reachable_walkable_rooted(anchor_starts(plan))`
/// — the set `DW1037`'s stance pass reads — computed only when some body needs
/// it.
pub fn check(
    plan: &Plan,
    world: &crate::compiler::nav::World,
) -> (AnswerBinding, Result<(), Failure>) {
    let c = plan.campaign;
    let mut b = AnswerBinding::default();
    let mut reachable: Option<BTreeSet<[i32; 3]>> = None;
    let mut readings: Vec<Reading<'_>> = Vec::new();
    for site in delvewright_dsl::body_sites(c) {
        let body = site.body;
        b.bodies += 1;
        let declared = body.answers().map_or(0, |a| {
            BodyPress::ALL
                .iter()
                .filter(|p| a.line(**p).is_some())
                .count()
        });
        b.declared += declared;
        let kind = body.worn_entity();
        let mut reading = Reading {
            body,
            path: site.path.clone(),
            owed: Vec::new(),
            reach: None,
        };
        if delvewright_dsl::removed_on_peaceful(kind) {
            b.monster += 1;
            if body.inert() {
                b.inert += 1;
                if body.struck_rider(c).is_some() {
                    b.carried += 1;
                } else {
                    let walk = reachable.get_or_insert_with(|| {
                        world.reachable_walkable_rooted(&crate::compiler::edit::anchor_starts(plan))
                    });
                    reading.reach = stations(plan, body).into_iter().find_map(|(cell, why)| {
                        reach_cells(world, &body_box(kind, cell), walk)
                            .first()
                            .map(|s| (why, cell, *s))
                    });
                    if reading.reach.is_some() {
                        b.in_reach += 1;
                        reading.owed = body.presses().to_vec();
                        b.owed += reading.owed.len();
                    } else {
                        b.out_of_reach += 1;
                    }
                }
            }
        }
        if reading.owed.is_empty() {
            b.volunteered += declared;
        }
        b.rows.push(AnswerRow {
            id: body.id().to_string(),
            kind: kind.to_string(),
            monster: delvewright_dsl::removed_on_peaceful(kind),
            inert: body.inert(),
            carried_by: body.struck_rider(c).map(|t| t.id.as_str().to_string()),
            reached: reading.reach.as_ref().map(|(_, s, c)| (*s, *c)),
            owed: reading.owed.iter().map(|p| p.token()).collect(),
            declared: BodyPress::ALL
                .iter()
                .filter(|p| body.answers().and_then(|a| a.line(**p)).is_some())
                .map(|p| p.token())
                .collect(),
        });
        readings.push(reading);
    }
    let mut lines = Vec::new();
    for r in &readings {
        let missing: Vec<BodyPress> = r
            .owed
            .iter()
            .copied()
            .filter(|p| r.body.answers().and_then(|a| a.line(*p)).is_none())
            .collect();
        if missing.is_empty() {
            continue;
        }
        b.refused += 1;
        let (why, cell, stance) = r.reach.clone().expect("an owed body was reached");
        let class = match r.body {
            BodyRef::Npc(_) => "npc",
            BodyRef::Actor(_) => "actor",
        };
        let local = r.body.id().split_once('/').map_or(r.body.id(), |(_, x)| x);
        let names: Vec<String> = missing
            .iter()
            .map(|p| {
                format!(
                    "`answers.{}` (key `{class}.{local}.answers.{}`, {})",
                    p.token(),
                    p.token(),
                    p.act()
                )
            })
            .collect();
        lines.push(format!(
            "\n  `{}` (`{}{}`, a `{}`) stands at {cell:?} ({why}) and a player standing on \
             {stance:?} can reach it; it cannot be harmed and does not fight, and the pinned game \
             discards a `{}` on peaceful — its own statement that the kind is a monster. A player \
             who strikes it meets a blow that does nothing and says nothing. Missing: {}.",
            r.body.id(),
            r.path,
            "/answers",
            r.body.worn_entity(),
            r.body.worn_entity(),
            names.join(", ")
        ));
    }
    let verdict = if lines.is_empty() {
        Ok(())
    } else {
        Err(Failure::new(
            DW_BODY_SILENT,
            format!(
                "{} harmless monster-kind bod(y/ies) stand where the party walks and say nothing \
                 when struck:{}\nPrescription: write each missing line in the campaign's own \
                 words — one `Refusal` saying what the thing is (`game-writing.md` F2) — under \
                 the body's `answers`; or stage the body where no walked route reaches it; or \
                 give it a `strike` trigger, if the blow is meant to do something.",
                lines.len(),
                lines.concat()
            ),
        ))
    };
    (b, verdict)
}
