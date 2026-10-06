//! **A blinding beside a drop** (spec-0085 §6): the proof a status-effect grant
//! that hides the floor owes the world it lands in.
//!
//! `DW0891` proves every killing volume shows itself to a player who can see. A
//! blinding takes the seeing away: a player who walked up to a pit's rim with
//! the pit in view and is then blinded stands beside a hazard they cannot see,
//! and the bot cannot fail it for them — mineflayer reads the server's blocks,
//! not the client's fog. That is the invisible-hazard class with the
//! invisibility moved from the floor to the eye, and the operating ruling's own
//! test applies: *can a body standing on solid ground be caught.* With the floor
//! hidden, a body that walks can be. So the refusal is owed to the **grant** — a
//! `give-effect` of a blinding effect ([`delvewright_dsl::perception::BLINDING`])
//! wherever it is written — never to the perception beat in particular.
//!
//! # The rule, in cells
//!
//! * the **standing set** `S`: where the players the grant reaches can stand
//!   when it lands — the standable cells of its `in` box, else what its root
//!   says ([`standing_set`]), widened by the moves a body can make before a
//!   delayed grant in a timeline lands;
//! * the **reach** `n`: `ceil(seconds × speed)` body moves, walking for an
//!   effect that forbids the sprint and sprinting for one that does not
//!   ([`delvewright_dsl::perception::reach_moves`]);
//! * the **blind reach** `R`: every cell a body gets to from `S` in at most `n`
//!   moves of [`World::body_moves`] — walk, survivable fall, jump — over the
//!   world with its exclusions lifted ([`World::without_exclusions`], the
//!   counterfactual `DW0891`'s population is measured over);
//! * **caught**: a cell of `R` inside a killing volume's keep-out, or a standing
//!   cell of `R` beside which a body steps off into a fall it does not survive
//!   ([`World::fatal_step_off`]).
//!
//! The remedy moves the blinding or the volume, never a cell of the walk.
//!
//! # What it does not catch
//!
//! A fall that hurts and does not kill; a player already blind from their own
//! potion; momentum a sprint carried into the blinding. The demo level is where
//! those are looked at.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::stages::{Objective, TrapTrigger, TriggerOn};
use delvewright_dsl::{DwCode, ExitTier, QuestEffect, Verb};

use crate::compiler::failure::{Failure, cells_by_floor};
use crate::compiler::nav::World;
use crate::compiler::plan::{EffectRoot, Plan};

/// An inclusive cell box, `(lo, hi)`.
type Box3 = ([i32; 3], [i32; 3]);

delvewright_dsl::dw_code! {
    /// `DW0943`: **a blinding grant whose reach meets a killing volume or a fatal
    /// drop** (spec-0085 §6).
    pub const DW_PERCEPTION_BLIND_REACH: DwCode = DwCode::new("DW0943", ExitTier::Build);
}

/// What the proof examined for one blinding grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrantReach {
    /// The stage document the grant lives in.
    pub stage: &'static str,
    /// JSON pointer to the grant.
    pub path: String,
    /// The blinding effect id.
    pub effect: String,
    /// The grant's `seconds`.
    pub seconds: u32,
    /// `|S|` — standable cells the grant's audience can stand in when it lands.
    pub standing: usize,
    /// `n` — body moves the grant reaches.
    pub reach_moves: usize,
    /// `|R|`.
    pub reached: usize,
    /// Caught cells of `R`, sorted (ADR-0006).
    pub caught: Vec<[i32; 3]>,
    /// What catches the first caught cell, in words.
    pub caught_by: Option<String>,
}

impl GrantReach {
    /// This grant's row of the ledger.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "stage": self.stage,
            "path": self.path,
            "effect": self.effect,
            "seconds": self.seconds,
            "standing": self.standing,
            "reach_moves": self.reach_moves,
            "reached": self.reached,
            "caught": self.caught,
            "caught_by": self.caught_by,
        })
    }
}

/// What the proof examined over the whole campaign.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlindReach {
    /// One row per blinding grant, in root-walk order.
    pub grants: Vec<GrantReach>,
}

impl BlindReach {
    /// The one line this proof owes its reader, zeroes included.
    pub fn line(&self) -> String {
        let list = |f: &dyn Fn(&GrantReach) -> usize| {
            if self.grants.is_empty() {
                "0".to_string()
            } else {
                let v: Vec<String> = self.grants.iter().map(|g| f(g).to_string()).collect();
                match v.len() {
                    1 => v[0].clone(),
                    _ => format!("{} and {}", v[..v.len() - 1].join(", "), v[v.len() - 1]),
                }
            }
        };
        format!(
            "blind-reach binding: {} blinding grant(s) examined; standing sets of {} cell(s); \
             reaches of {} move(s); {} caught.",
            self.grants.len(),
            list(&|g| g.standing),
            list(&|g| g.reach_moves),
            self.grants.iter().filter(|g| !g.caught.is_empty()).count(),
        )
    }

    /// The ledger's `blind_reach` object.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "grants_examined": self.grants.len(),
            "caught": self.grants.iter().filter(|g| !g.caught.is_empty()).count(),
            "grants": self.grants.iter().map(GrantReach::to_json).collect::<Vec<_>>(),
        })
    }
}

/// Where a nested bundle's audience stands, when it is not its root's answer.
#[derive(Clone, Copy)]
enum Seat<'a> {
    /// The root's own answer.
    Root,
    /// A respawn or rest seat at this anchor.
    Anchor(&'a str),
    /// The whole walked population: a bundle whose firing time or place the
    /// static model does not have (an `on_arrive`, an `on_caught`).
    Anywhere,
}

/// One blinding grant found in the walk, with what decides its standing set.
struct Found<'a> {
    stage: &'static str,
    path: String,
    eff: &'a QuestEffect,
    root: EffectRoot<'a>,
    seat: Seat<'a>,
    delay_ticks: u32,
    /// Whether, absent an `audience`, the bundle addresses its actor (`@s`)
    /// rather than the party.
    solo: bool,
}

/// Every blinding grant in the campaign, through the one root walk and the one
/// nesting authority.
fn grants<'a>(plan: &Plan<'a>) -> Vec<Found<'a>> {
    /// What a nesting site hands down to the effects inside it.
    #[derive(Clone, Copy)]
    struct Ctx<'a> {
        stage: &'static str,
        root: EffectRoot<'a>,
        seat: Seat<'a>,
        delay: u32,
        solo: bool,
    }
    fn descend<'a>(ctx: Ctx<'a>, path: String, eff: &'a QuestEffect, out: &mut Vec<Found<'a>>) {
        if let Some((effect, _, _, _, _)) = eff.give_effect()
            && delvewright_dsl::perception::blinding(effect).is_some()
        {
            out.push(Found {
                stage: ctx.stage,
                path: path.clone(),
                eff,
                root: ctx.root,
                seat: ctx.seat,
                delay_ticks: ctx.delay,
                solo: ctx.solo,
            });
        }
        match &eff.verb {
            Verb::Sequence { steps } => {
                for (s, st) in steps.iter().enumerate() {
                    let inner_ctx = Ctx {
                        delay: ctx.delay + st.at_ticks,
                        ..ctx
                    };
                    for (j, inner) in st.effects.iter().enumerate() {
                        descend(
                            inner_ctx,
                            format!("{path}/steps/{s}/effects/{j}"),
                            inner,
                            out,
                        );
                    }
                }
            }
            Verb::SetCheckpoint { anchor, on_respawn } => {
                // The respawning player's own bundle, at the seat.
                let inner_ctx = Ctx {
                    seat: Seat::Anchor(anchor.as_str()),
                    delay: 0,
                    solo: true,
                    ..ctx
                };
                for (j, inner) in on_respawn.iter().enumerate() {
                    descend(inner_ctx, format!("{path}/on_respawn/{j}"), inner, out);
                }
            }
            Verb::Bonfire {
                anchor, on_rest, ..
            } => {
                // A rest addresses the whole party from the tick.
                let inner_ctx = Ctx {
                    seat: Seat::Anchor(anchor.as_str()),
                    delay: 0,
                    solo: false,
                    ..ctx
                };
                for (j, inner) in on_rest.iter().enumerate() {
                    descend(inner_ctx, format!("{path}/on_rest/{j}"), inner, out);
                }
            }
            _ => {
                let solos = eff
                    .nested_effect_dispatch()
                    .into_iter()
                    .map(|(_, how)| how == delvewright_dsl::NestedDispatch::Player);
                for ((pseg, _k, list), solo) in
                    eff.nested_effect_lists_labeled().into_iter().zip(solos)
                {
                    let inner_ctx = Ctx {
                        seat: Seat::Anywhere,
                        delay: 0,
                        solo,
                        ..ctx
                    };
                    for (j, inner) in list.iter().enumerate() {
                        descend(inner_ctx, format!("{path}/{pseg}/{j}"), inner, out);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    crate::compiler::plan::for_each_effect_root(plan.campaign, &mut |site, effs| {
        let ctx = Ctx {
            stage: site.stage,
            root: site.root,
            seat: Seat::Root,
            delay: 0,
            solo: root_is_solo(&site.root),
        };
        for (i, eff) in effs.iter().enumerate() {
            descend(ctx, format!("{}/{i}", site.path), eff, &mut out);
        }
    });
    out
}

/// Whether a root's bundle addresses its actor when an effect states no
/// `audience` — the emitter's `Audience::Solo` roots.
fn root_is_solo(root: &EffectRoot<'_>) -> bool {
    match root {
        EffectRoot::Trigger(t) => t.addresses_presser(),
        EffectRoot::DialogueRespawn
        | EffectRoot::OnDeath
        | EffectRoot::ShopOffer
        | EffectRoot::OnKill(_) => true,
        // spec-0082: a strike's landing runs from the assembly's scheduled
        // tick (`Audience::Scheduled`), with no acting player.
        EffectRoot::AssemblyLand(_)
        // spec-0086: a loop's `on_cross` runs from the server source after the
        // move (`Audience::Scheduled`).
        | EffectRoot::LoopCross(_)
        | EffectRoot::ObjectiveComplete { .. }
        | EffectRoot::QuestComplete(_)
        | EffectRoot::TrapPayload(_)
        | EffectRoot::ShortcutUnlock => false,
    }
}

/// Standable cells of `open` whose cell lies within `radius` (Euclidean, cell to
/// cell) of `at`.
fn standable_within(open: &World, at: [i32; 3], radius: f64) -> BTreeSet<[i32; 3]> {
    let r = radius.ceil() as i32;
    let mut out = BTreeSet::new();
    for dx in -r..=r {
        for dy in -r..=r {
            for dz in -r..=r {
                if f64::from(dx * dx + dy * dy + dz * dz) > radius * radius {
                    continue;
                }
                let c = [at[0] + dx, at[1] + dy, at[2] + dz];
                if open.is_standable(c) {
                    out.insert(c);
                }
            }
        }
    }
    out
}

/// **Where a grant's audience can stand when it lands** (spec-0085 §6.2's
/// table), before a timeline's delay widens it.
///
/// The table's per-root rows are where the ACTOR stands, so they answer only a
/// grant addressed to the actor. A grant addressed to the party — a stated
/// `party`, or the default of a root that addresses the party — blinds every
/// player wherever they are, and its standing set is the walked population `P`
/// whatever its root. (Authored beyond the spec's table, which reads every row
/// as the actor's; the party reading is the one the emitted `@a` makes true.)
///
/// Every root the table names by a narrower set is answered by that set; every
/// root it answers with the walked population `P` — and every place this model
/// has no narrower answer for (a shop, a strike on an NPC, an `on_arrive`) — is
/// answered by `P`, the largest set there is, so a fallback can only refuse
/// more, never less.
fn standing_set(
    plan: &Plan,
    open: &World,
    population: &BTreeSet<[i32; 3]>,
    f: &Found<'_>,
) -> BTreeSet<[i32; 3]> {
    if let Some(zone) = f.eff.within.as_ref() {
        let Some((lo, hi)) = plan.zone_box(zone) else {
            return BTreeSet::new();
        };
        let mut out = BTreeSet::new();
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    if open.is_standable([x, y, z]) {
                        out.insert([x, y, z]);
                    }
                }
            }
        }
        return out;
    }
    // A grant addressed to the party blinds every player wherever they stand,
    // so where its root fired says nothing about where they are.
    let to_actor = match f.eff.audience {
        Some(delvewright_dsl::EffectAudience::Actor) => true,
        Some(delvewright_dsl::EffectAudience::Party) => false,
        None => f.solo,
    };
    if !to_actor {
        return population.clone();
    }
    let reach = crate::compiler::crosshair::INTERACTION_REACH + 1.0;
    match f.seat {
        Seat::Anywhere => return population.clone(),
        Seat::Anchor(a) => {
            return plan
                .point_any(a)
                .and_then(|p| open.snap(p, crate::compiler::nav::SNAP_RADIUS))
                .into_iter()
                .collect();
        }
        Seat::Root => {}
    }
    match f.root {
        EffectRoot::Trigger(t) => match (&t.on, &t.at) {
            (TriggerOn::Approach { range }, Some(at)) => plan
                .point_any(at.as_str())
                .map(|p| standable_within(open, p, f64::from(*range)))
                .unwrap_or_default(),
            (TriggerOn::Use | TriggerOn::Strike, Some(at)) => plan
                .point_any(at.as_str())
                .map(|p| standable_within(open, p, reach))
                .unwrap_or_default(),
            _ => population.clone(),
        },
        EffectRoot::TrapPayload(trap) => {
            let Some(p) = plan.point_any(trap.at.as_str()) else {
                return BTreeSet::new();
            };
            match trap.trigger {
                // The body stands on the plate or the wire, or straddles it.
                TrapTrigger::PressurePlate | TrapTrigger::Tripwire => {
                    standable_within(open, p, 1.0)
                }
                TrapTrigger::TrappedChest => standable_within(open, p, reach),
            }
        }
        EffectRoot::ObjectiveComplete { objective, .. } => {
            let site = crate::compiler::reach::sites(plan)
                .into_iter()
                .find(|s| s.objective_id == objective);
            let is_reach = plan.campaign.quests.content.quests.iter().any(|q| {
                q.objectives.iter().any(|o| {
                    o.id().as_str() == objective && matches!(o, Objective::ReachAnchor { .. })
                })
            });
            match (is_reach, site) {
                (true, Some(s)) => {
                    crate::compiler::reach::ReachJudgement::take(open, population, s.pos, s.radius)
                        .footprint
                }
                _ => population.clone(),
            }
        }
        EffectRoot::DialogueRespawn | EffectRoot::OnDeath => plan
            .checkpoints
            .iter()
            .filter_map(|c| open.snap(c.pos, crate::compiler::nav::SNAP_RADIUS))
            .collect(),
        EffectRoot::QuestComplete(_)
        | EffectRoot::ShortcutUnlock
        | EffectRoot::ShopOffer
        | EffectRoot::OnKill(_)
        // spec-0082: a landing addresses the party from the assembly's tick;
        // every cell the party may stand on is a seat, which can only refuse
        // more.
        | EffectRoot::AssemblyLand(_)
        // spec-0086: an `on_cross` addresses the party (`@a`) from the loop's
        // tick; every cell the party may stand on is a seat.
        | EffectRoot::LoopCross(_) => population.clone(),
    }
}

/// Every cell a body gets to from `from` in at most `moves` body moves, with its
/// depth.
fn flood(open: &World, from: &BTreeSet<[i32; 3]>, moves: usize) -> BTreeMap<[i32; 3], usize> {
    let mut seen: BTreeMap<[i32; 3], usize> = from.iter().map(|&c| (c, 0)).collect();
    let mut queue: VecDeque<[i32; 3]> = from.iter().copied().collect();
    while let Some(c) = queue.pop_front() {
        let d = seen[&c];
        if d >= moves {
            continue;
        }
        for n in open.body_moves(c) {
            if let std::collections::btree_map::Entry::Vacant(v) = seen.entry(n) {
                v.insert(d + 1);
                queue.push_back(n);
            }
        }
    }
    seen
}

/// `DW0943`: **prove no blinding grant's reach meets a killing volume or a fatal
/// drop** (spec-0085 §6).
///
/// Asked after `DW0891`, so every volume it reasons about is one the player
/// could see, and before the route proofs. Returns the binding beside the
/// verdict, so the line a run prints counts every grant rather than the ones
/// before the failure.
pub fn check(
    plan: &Plan,
    world: &World,
    entry: Option<[i32; 3]>,
) -> (BlindReach, Result<(), Failure>) {
    // The counterfactual, never the world the router walks: the walk model
    // already refuses the keep-out, so a reach taken over it could never meet
    // one and this check would be green while binding to nothing.
    judge(plan, &world.without_exclusions(), entry)
}

/// [`check`]'s judgement over the world the caller hands it as `open`. Public so
/// the vacuity test can hand it the lethal-APPLIED world and watch a caught
/// grant go green — the shape [`check`] exists not to have.
pub fn judge(
    plan: &Plan,
    open: &World,
    entry: Option<[i32; 3]>,
) -> (BlindReach, Result<(), Failure>) {
    let mut binding = BlindReach::default();
    let found = grants(plan);
    if found.is_empty() {
        return (binding, Ok(()));
    }
    let population = open.reachable_walkable(&crate::compiler::lethal::put_at_roots(plan, entry));
    let body = delvewright_dsl::metrics::Body::PLAYER;
    let keep_outs: Vec<(&str, Box3)> = plan
        .lethal_volumes
        .iter()
        .map(|v| {
            (
                v.id.as_str(),
                delvewright_dsl::metrics::keep_out_box(body, v.region.0, v.region.1),
            )
        })
        .collect();
    let inside = |c: [i32; 3], (lo, hi): ([i32; 3], [i32; 3])| {
        (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i])
    };
    let mut verdict: Option<Failure> = None;
    for f in &found {
        let Some((effect, seconds, _, _, _)) = f.eff.give_effect() else {
            continue;
        };
        let forbids_sprint = delvewright_dsl::perception::blinding(effect).unwrap_or(true);
        let mut standing = standing_set(plan, open, &population, f);
        // A delayed grant lands on a body that has had the delay to move, with
        // its eyes open — at the sprint. An `in` box is judged at the moment it
        // fires, so it is not widened.
        if f.delay_ticks > 0 && f.eff.within.is_none() {
            let pre = delvewright_dsl::perception::reach_moves(f.delay_ticks.div_ceil(20), false);
            standing = flood(open, &standing, pre).into_keys().collect();
        }
        let n = delvewright_dsl::perception::reach_moves(seconds, forbids_sprint);
        let reach = flood(open, &standing, n);
        let mut caught: Vec<[i32; 3]> = Vec::new();
        let mut caught_by: Option<String> = None;
        for &c in reach.keys() {
            if let Some((id, k)) = keep_outs.iter().find(|(_, k)| inside(c, *k)) {
                caught.push(c);
                caught_by.get_or_insert_with(|| {
                    format!(
                        "lethal volume `{id}`'s keep-out {:?}..={:?} holds {c:?}",
                        k.0, k.1
                    )
                });
                continue;
            }
            if open.is_standable(c)
                && let Some((side, lava)) = open.fatal_step_off(c)
            {
                caught.push(c);
                caught_by.get_or_insert_with(|| {
                    if lava {
                        format!("beside {c:?} a body steps into lava at {side:?}")
                    } else {
                        format!(
                            "beside {c:?} a body steps off into column {side:?}, which arrests \
                             no fall within the {} block(s) an unarmoured body survives",
                            delvewright_dsl::metrics::unarmoured_survivable_fall_blocks()
                        )
                    }
                });
            }
        }
        let row = GrantReach {
            stage: f.stage,
            path: f.path.clone(),
            effect: effect.to_string(),
            seconds,
            standing: standing.len(),
            reach_moves: n,
            reached: reach.len(),
            caught: caught.clone(),
            caught_by: caught_by.clone(),
        };
        if verdict.is_none() {
            if standing.is_empty() && f.eff.within.is_some() {
                verdict = Some(Failure {
                    code: DW_PERCEPTION_BLIND_REACH,
                    message: format!(
                        "blinding grant `{effect}` at {} {} is narrowed by an `in` box over 0 \
                         standable cell(s): the declaration says players stand there when it \
                         lands, and the assembled bytes hold no cell a body can stand in, so the \
                         proof that the grant's reach stops short of every hazard would bind to \
                         nothing. Draw the box over the floor the players stand on, or delete \
                         the grant.",
                        f.stage, f.path
                    ),
                });
            } else if !caught.is_empty() {
                verdict = Some(Failure {
                    code: DW_PERCEPTION_BLIND_REACH,
                    message: format!(
                        "blinding grant `{effect}` for {seconds} s at {} {} hides the floor from \
                         players standing in {} cell(s); in {n} move(s) — {} — a body reaches \
                         {} cell(s), and {} of them can be caught: {}. First: {}. Danger is \
                         visible, or the engine refuses it, and a blinded player cannot see the \
                         hazard beside them. Shorten `seconds` so the reach stops short; draw \
                         `in` so the standing set is farther from the hazard; use \
                         `minecraft:nausea`, which leaves the floor visible; or move the volume \
                         as DW0891's remedy says. Never remove a cell from the walk.",
                        f.stage,
                        f.path,
                        standing.len(),
                        if forbids_sprint {
                            "walking, since it forbids the sprint"
                        } else {
                            "at the sprint, which it does not forbid"
                        },
                        reach.len(),
                        caught.len(),
                        cells_by_floor(&caught),
                        caught_by.unwrap_or_default(),
                    ),
                });
            }
        }
        binding.grants.push(row);
    }
    (binding, verdict.map_or(Ok(()), Err))
}
