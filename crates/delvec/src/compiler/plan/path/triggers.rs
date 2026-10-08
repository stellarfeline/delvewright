//! The triggers the forced walk performs, and the steps at which beats fire.

use super::*;

/// **The environment triggers a path performs, and where** — keyed by the index
/// (into `path.steps`) of the objective step each is performed in front of.
///
/// A trigger is a party action nothing on the quest DAG orders, and the proofs
/// credit two things it does: the way it opens (the region-write model) and the
/// flags it sets (the flow replay). A path that is credited with either owes the
/// act that produces it, or the bot walks up to a wall the proof called open. So
/// a path performs:
///
/// * every trigger whose bundle **opens a way** — an `open-gate` or an
///   `open-way`, the two verbs whose one meaning is that a threshold stands open
///   ([`opens_a_way`]) — at the first step where its flag gate holds
///   (`requires_flags` held, no `forbids_flags` set) and the party is in the area
///   its target stands in; and
/// * every trigger that pays a **flag debt** ([`crate::compiler::flow::Flow::trigger_debts`]),
///   at that same first point if it comes no later than the step that reads the
///   flag, otherwise immediately in front of the reader.
///
/// A `clear-region` is a general region write (a lift's car, a collapsing floor),
/// not a threshold, so it does not make a trigger performed by itself: pressing
/// every lever in a room is not what a player does. It is credited when the path
/// performs its trigger for one of the two reasons above, and otherwise not at
/// all.
///
/// A trigger whose gate never holds on this path, or whose target never shares
/// an area with a step of it, is not performed — and then nothing it opens is
/// credited ([`collect_region_events`]), which is the direction that can only
/// turn a proof red. Numeric gates (`requires_state`) are not evaluated here: a
/// trigger performed while one does not hold never broadcasts its marker, and
/// the step fails where it stands rather than further on.
pub(super) fn path_triggers(
    campaign: &Campaign,
    anchors: &AnchorTable,
    flow: &crate::compiler::flow::Flow<'_>,
    path: &crate::compiler::flow::Playthrough,
    flags_at: &[BTreeSet<String>],
    begun: &BTreeSet<String>,
) -> BTreeMap<usize, Vec<Step>> {
    let mut out: BTreeMap<usize, Vec<Step>> = BTreeMap::new();
    if path.steps.is_empty() {
        return out;
    }
    // Which triggers open a way: the SAME walk the region-write model credits
    // them from, so the two cannot disagree about what a trigger's bundle holds.
    let mut opens: BTreeSet<&str> = BTreeSet::new();
    for_each_gate_effect(campaign, &mut |site, e| {
        if let EffectRoot::Trigger(t) = site.root
            && opens_a_way(e)
        {
            opens.insert(t.id.as_str());
        }
    });
    let debts = flow.trigger_debts(path);
    let area_of = |si: usize| -> &str {
        let qid = path.steps[si].quest.as_str();
        campaign
            .quest_plan
            .content
            .quests
            .iter()
            .find(|q| q.id.as_str() == qid)
            .map(|q| q.area.as_str())
            .unwrap_or("")
    };
    for t in &campaign.quests.content.triggers {
        let reader = debts
            .iter()
            .find(|d| d.trigger == t.id.as_str())
            .map(|d| d.step);
        if reader.is_none() && !opens.contains(t.id.as_str()) {
            continue;
        }
        // The flags this trigger's own bundle sets. The journal credits a
        // trigger-paid flag as held from the step it is owed at, so a trigger
        // that stands itself down with the flag it pays — a hit count that
        // forbids its own completion flag (spec-0082 §3.3) — would read as
        // forbidden before it was ever performed. Its own flags are not yet
        // held when the path decides to perform it.
        let mut own: BTreeSet<&str> = BTreeSet::new();
        for e in &t.effects {
            e.visit_deep(&mut |x| {
                if let delvewright_dsl::Verb::SetFlag { flag, .. } = &x.verb {
                    own.insert(flag.as_str());
                }
            });
        }
        let enabled = |si: usize| {
            let held = &flags_at[si];
            t.requires_flags.iter().all(|f| held.contains(f.as_str()))
                && !t
                    .forbids_flags
                    .iter()
                    .any(|f| held.contains(f.as_str()) && !own.contains(f.as_str()))
        };
        // Where the target stands when the party is at step `si`: the area the
        // emitter summoned its body in, the NPC's body at this beat, or the
        // struck assembly's mark (spec-0082).
        // An assembly stands only from the path step whose completion bundle
        // spawns it: struck before then, there is nothing there to strike.
        let spawned_at: Option<usize> = t.on.assembly_target().and_then(|a| {
            path.steps.iter().position(|st| {
                let Some(q) = campaign
                    .quests
                    .content
                    .quests
                    .iter()
                    .find(|q| q.id.as_str() == st.quest)
                else {
                    return false;
                };
                let mut spawns = false;
                let mut look = |effs: &[QuestEffect]| {
                    for e in effs {
                        e.visit_deep(&mut |x| {
                            spawns |= matches!(&x.verb,
                                delvewright_dsl::Verb::SpawnAssembly { assembly } if assembly == a);
                        });
                    }
                };
                if let Some((_, effs)) = q
                    .on_objective_complete
                    .iter()
                    .find(|(k, _)| k.as_str() == st.objective.as_str())
                {
                    look(effs);
                }
                let last =
                    q.objectives.last().map(|o| o.id().as_str()) == Some(st.objective.as_str());
                if last {
                    look(&q.on_complete);
                }
                spawns
            })
        });
        let target = |si: usize| -> Option<(String, [i32; 3])> {
            if let Some(a) = t.on.assembly_target() {
                if !spawned_at.is_some_and(|s| s < si) {
                    return None;
                }
                let decl = campaign.quests.content.assembly_decl(a.as_str())?;
                let (area, cell) = anchors
                    .iter()
                    .find(|((_, n), _)| n == decl.at.anchor.as_str())
                    .map(|((a, _), r)| {
                        (
                            a.clone(),
                            match r {
                                ResolvedAnchor::Point { pos, .. } => *pos,
                                ResolvedAnchor::Gate { from, .. } => *from,
                            },
                        )
                    })?;
                // The mark: where the thing stands, the cell a party walks up to.
                // Whether its hitbox is within a strike of anywhere the party
                // can stand is `DW0937`'s question, asked of the box itself.
                return Some((area, decl.at.cell(cell)));
            }
            match t.on.npc_target() {
                Some(npc) => npc_beat_cell(
                    campaign,
                    anchors,
                    npc.as_str(),
                    path.steps[si].quest.as_str(),
                    area_of(si),
                    begun,
                    &flags_at[si],
                ),
                None => {
                    let at = t.at_anchor()?;
                    anchors
                        .iter()
                        .find(|((_, n), _)| n == at)
                        .map(|((a, _), r)| {
                            (
                                a.clone(),
                                match r {
                                    ResolvedAnchor::Point { pos, .. } => *pos,
                                    ResolvedAnchor::Gate { from, .. } => *from,
                                },
                            )
                        })
                }
            }
        };
        let last = reader.unwrap_or(path.steps.len() - 1);
        let at = (0..=last)
            .find(|&si| enabled(si) && target(si).is_some_and(|(a, _)| a == area_of(si)))
            .or_else(|| reader.filter(|&r| enabled(r)));
        let Some(si) = at else { continue };
        let Some((_, pos)) = target(si) else { continue };
        // A count is N performances (spec-0082 §10): a repeatable trigger whose
        // way-opening or flag-paying effect waits on a datum the trigger itself
        // counts up is performed as often as the count needs.
        for _ in 0..fires_needed(campaign, t) {
            out.entry(si).or_default().push(Step::Trigger {
                trigger_id: t.id.as_str().to_string(),
                on: t.on.kind(),
                anchor_id: t.at_anchor().map(str::to_string),
                npc_id: t.on.npc_target().map(|n| n.as_str().to_string()),
                assembly_id: t.on.assembly_target().map(|a| a.as_str().to_string()),
                pos,
                range: match t.on {
                    delvewright_dsl::TriggerOn::Approach { range } => Some(range),
                    _ => None,
                },
                stand: None,
                block: pressed_block(campaign, t.id.as_str()),
            });
        }
    }
    out
}

/// How many times a path performs `t` before the effect it is performed for
/// happens: 1, unless `t` is repeatable and an effect that opens a way or sets
/// a flag is gated on a datum `t`'s own bundle counts up ahead of it — a hit
/// count (spec-0082 §3.3). Then it is the least `n` for which the datum's
/// declared initial plus `n` times the bundle's ungated `add-state` amounts
/// satisfies every term of that gate on it. A gate the count cannot open
/// within 64 performances, or one that reads anything the bundle does not
/// count, is left at 1 — the path performs the trigger and the step fails
/// where it stands, the direction that can only turn a proof red.
fn fires_needed(campaign: &Campaign, t: &delvewright_dsl::EnvTrigger) -> usize {
    if t.once {
        return 1;
    }
    let mut needed = 1usize;
    let mut counted: BTreeMap<&str, i64> = BTreeMap::new();
    for e in &t.effects {
        if let delvewright_dsl::Verb::AddState { state, amount } = &e.verb
            && e.when.is_none()
        {
            *counted.entry(state.as_str()).or_insert(0) += i64::from(*amount);
            continue;
        }
        let pays = opens_a_way(e) || matches!(e.verb, delvewright_dsl::Verb::SetFlag { .. });
        let terms = e.requires_state();
        if !pays || terms.is_empty() {
            continue;
        }
        let holds = |n: i64| {
            terms.iter().all(|c| {
                let Some(step) = counted.get(c.state.as_str()).copied() else {
                    return false;
                };
                let Some(decl) = campaign.quests.content.state_decl(c.state.as_str()) else {
                    return false;
                };
                let v = i64::from(decl.initial) + n * step;
                i32::try_from(v).is_ok_and(|v| c.holds(v))
            })
        };
        if let Some(n) = (1..=64).find(|n| holds(*n)) {
            needed = needed.max(n as usize);
        }
    }
    needed
}

/// Whether a critical path could ever perform this trigger — its bundle opens a
/// way or sets a flag, the only two things [`path_triggers`] performs a trigger
/// for; it hosts a **link** (a repeatable trigger carrying a `teleport`,
/// spec-0083 §3.1), which the route proof performs where a walk fails; or it is
/// pressed by hand and writes a datum, which a numeric gate may owe
/// (`super::drive`). The
/// emitter broadcasts a fired marker from exactly these, so every `trigger`
/// step has a line to pass on and no other trigger prints one.
pub(crate) fn trigger_may_be_performed(t: &delvewright_dsl::EnvTrigger) -> bool {
    fn carries(effs: &[QuestEffect]) -> bool {
        effs.iter().any(|e| {
            e.teleport().is_some()
                || matches!(&e.verb, delvewright_dsl::Verb::Sequence { steps }
                    if steps.iter().any(|s| carries(&s.effects)))
        })
    }
    if !t.once && carries(&t.effects) {
        return true;
    }
    // A press that moves a datum: a numeric gate may owe it (`DW0985`).
    if super::drive::pressable(t) && super::drive::writes_state_at_top(t) {
        return true;
    }
    fn deep(effs: &[QuestEffect]) -> bool {
        effs.iter().any(|e| {
            opens_a_way(e)
                || matches!(e.verb, delvewright_dsl::Verb::SetFlag { .. })
                || e.nested_effect_lists_labeled()
                    .into_iter()
                    .any(|(_, _, list)| deep(list))
        })
    }
    deep(&t.effects)
}

/// Whether an effect opens a way on: an `open-gate` or an `open-way`.
fn opens_a_way(e: &QuestEffect) -> bool {
    matches!(e.gate_region_write(), Some((_, false))) || e.way_write().is_some()
}

/// Where an NPC's body stands for the beat of quest `qid` — the cast ledger's
/// station for this beat when it has one, otherwise the stage-2 declaration —
/// as `(area, cell)`. The position half of the `talk-to` step's resolution,
/// without its refusals: those belong to the `talk-to` that owns the beat, and a
/// `strike-npc` trigger aimed at a body the ledger cannot place simply has no
/// target, so the path does not perform it.
fn npc_beat_cell(
    campaign: &Campaign,
    anchors: &AnchorTable,
    npc: &str,
    qid: &str,
    area: &str,
    begun: &BTreeSet<String>,
    flags: &BTreeSet<String>,
) -> Option<(String, [i32; 3])> {
    let decl = campaign
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc)?;
    let home = decl.area.as_str();
    match crate::compiler::cast::station(campaign, npc, qid, begun, flags) {
        Some(crate::compiler::cast::Station::At(anchor, offset)) => {
            body_station(anchors, BodyScope::Beat { beat: area, home }, anchor)
                .place()
                .map(|(a, p)| (a.to_string(), delvewright_dsl::offset_cell(p, offset)))
        }
        Some(crate::compiler::cast::Station::Absent(_)) => None,
        None => anchors
            .get(&(home.to_string(), decl.anchor.as_str().to_string()))
            .map(|r| {
                let p = match r {
                    ResolvedAnchor::Point { pos, .. } => *pos,
                    ResolvedAnchor::Gate { from, .. } => *from,
                };
                (
                    home.to_string(),
                    delvewright_dsl::offset_cell(p, decl.offset),
                )
            }),
    }
}

/// The `critical_path` step index at which a quest's `on_complete` fires: its
/// last objective's step (max over the quest's objectives). `0` if the quest has
/// no positioned objective (degenerate; conservative — proves the whole path).
pub(crate) fn quest_complete_step(quest: &Quest, obj_step: &BTreeMap<String, usize>) -> usize {
    quest
        .objectives
        .iter()
        .filter_map(|o| obj_step.get(o.id().as_str()).copied())
        .max()
        .unwrap_or(0)
}

/// The block a trigger's act uses, when its `prop` is one a hand presses
/// (spec-0093 §6.5) — what a `trigger` step hands the harness to right-click.
pub(super) fn pressed_block(campaign: &Campaign, trigger_id: &str) -> Option<String> {
    campaign
        .quests
        .content
        .triggers
        .iter()
        .find(|t| t.id.as_str() == trigger_id)
        .and_then(|t| t.prop.as_ref())
        .filter(|p| p.is_hand_pressed())
        .map(|p| p.block.clone())
}

/// The `critical_path` step index of the `talk-to` objective that a dialogue tree
/// belongs to (its NPC's completing beat), rooting a dialogue-hosted
/// `set-checkpoint`. `0` if none is found (degenerate).
pub(in crate::compiler::plan) fn dialogue_fire_step(
    campaign: &Campaign,
    npc_id: &str,
    obj_step: &BTreeMap<String, usize>,
) -> usize {
    campaign
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| q.objectives.iter())
        .filter_map(|o| match o {
            Objective::TalkTo { id, npc, .. } if npc.as_str() == npc_id => {
                obj_step.get(id.as_str()).copied()
            }
            _ => None,
        })
        .min()
        .unwrap_or(0)
}
