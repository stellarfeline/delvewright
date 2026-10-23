//! **A click target always answers** (spec-0117): the one authority for what a
//! player presses — the real block every interaction point is, how it is
//! pressed, where its click box stands in each of its states, and the answer it
//! gives every press outside its window.
//!
//! ## The class
//!
//! An interaction point is a **real block at a cell** ([`PressPlan`]), placed at
//! `setup_finish` and standing for the whole delve, drawn and occluded like the
//! world. The block decides how it is pressed ([`delvewright_dsl::press_form`]):
//!
//! * **on its own** — a bell, a button, a lever whose every press acts: vanilla
//!   reports the press (`minecraft:default_block_use` at the cell) and it
//!   changes nothing the world keeps, so nothing is summoned;
//! * **through a click box** — every other block: an invisible
//!   `minecraft:interaction` fitted to the block's own outline
//!   ([`crate::compiler::clickbox::ClickBox::fitted_to`]) stands over it, so the
//!   crosshair's pick lands on the box and the block does not act. A block whose
//!   press acts on its own (a door, a lever with a shut or settled state, a
//!   chest) is boxed outside its window and bare inside it; a block the game
//!   reports no press of (a campfire, stone) and every `strike` is boxed in
//!   every state.
//!
//! Every interaction point the engine arms is in [`derive`]: the `interact`
//! objective, a `use` or `strike` trigger at a point, a gate's own answer, a
//! shortcut's unlock, a bonfire, a shop, a trap's and a timed gate's disarm. A
//! body a player strikes or speaks to — an NPC's dialogue hitbox, an assembly's
//! struck body — is a creature, not an interaction point, and keeps its own
//! box; the recovery stake's drop is the one interaction point that stays an
//! entity (spec-0117 §11).
//!
//! ## The answers
//!
//! Each declared answer is lowered into an ordinary presser trigger
//! ([`AnswerPlan::trigger`], `trigger/dw-answer-<owner>-<state>`) on the owner's
//! block, gated to the one state it answers, carrying one `narrate` on the
//! action bar: the dispatch, the l10n path and the PackTest are the ones every
//! presser trigger already has. The state is computed by one function per owner
//! (`pst_<owner>`, emitted by `emit::press`), read by every answer and by the
//! box reconciliation, so the act and its answers cannot disagree about which
//! state a press is in.

use std::collections::BTreeMap;

use crate::compiler::plan::{Plan, ResolvedAnchor, safe_local};
use delvewright_dsl::{
    AnswerState, Answers, DwCode, EnvTrigger, ExitTier, Highlight, Objective, PressForm, PressKind,
    Serves, TriggerOn,
};

delvewright_dsl::dw_code! {
    /// `DW0426`: a click trigger is anchored where a player can never click.
    ///
    /// The unbound-vacuity class, as a diagnostic. The trigger declares an anchor, a
    /// click and a full effect bundle; validation is happy, emission runs, and the
    /// press lands on nothing — so the beat simply never happens and every board
    /// stays green.
    pub const DW_TRIGGER_UNPRESSABLE: DwCode = DwCode::new("DW0426", ExitTier::Build);
}

/// What one interaction point is (the owner of its block and its answers).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Owner {
    /// An `interact` objective: quest index, objective index, its id.
    Interact {
        /// Index of the quest in `quests[]`.
        quest: usize,
        /// Index of the objective in the quest.
        objective: usize,
        /// The objective id.
        id: String,
    },
    /// An authored `use` or `strike` trigger at a point anchor.
    Trigger {
        /// The trigger id.
        id: String,
    },
    /// A gate's own answer: a `use` trigger with `serves: {gate}` anchored at a
    /// cell of the gate region; its block is the gate's block there.
    GateAnswer {
        /// The trigger id.
        id: String,
        /// The gate anchor it serves.
        gate: String,
    },
    /// A shortcut's unlock.
    Shortcut {
        /// The shortcut id.
        id: String,
    },
    /// A bonfire, by its checkpoint index.
    Bonfire {
        /// The bonfire's checkpoint index.
        index: usize,
    },
    /// A shop, by its declaration index.
    Shop {
        /// The shop's index in `shops[]`.
        index: usize,
    },
    /// A trap's disarm.
    TrapDisarm {
        /// The trap id.
        id: String,
    },
    /// A timed gate's disarm.
    TimedGateDisarm {
        /// The timed gate id.
        id: String,
    },
}

impl Owner {
    /// The token an answer's id and the state holder are spelled with:
    /// `obj-<local>`, `trig-<local>`, `sc-<local>`, `bf-<i>`, `shop-<i>`,
    /// `trapdis-<local>`, `tgdis-<local>`, `gate-<local>`.
    pub fn token(&self) -> String {
        let local = |id: &str| id.split_once('/').map_or(id, |(_, r)| r).to_string();
        match self {
            Owner::Interact { id, .. } => format!("obj-{}", local(id)),
            Owner::Trigger { id } => format!("trig-{}", local(id)),
            Owner::GateAnswer { id, .. } => format!("gate-{}", local(id)),
            Owner::Shortcut { id } => format!("sc-{}", local(id)),
            Owner::Bonfire { index } => format!("bf-{index}"),
            Owner::Shop { index } => format!("shop-{index}"),
            Owner::TrapDisarm { id } => format!("trapdis-{}", local(id)),
            Owner::TimedGateDisarm { id } => format!("tgdis-{}", local(id)),
        }
    }

    /// The token as a function- and score-safe name (`obj_pry`).
    pub fn safe(&self) -> String {
        self.token().replace(['-', '.', '/'], "_")
    }

    /// The words a message and the ledger use.
    pub fn word(&self) -> String {
        match self {
            Owner::Interact { id, .. } => format!("`interact` objective `{id}`"),
            Owner::Trigger { id } => format!("trigger `{id}`"),
            Owner::GateAnswer { id, gate } => format!("`{gate}`'s own answer `{id}`"),
            Owner::Shortcut { id } => format!("shortcut `{id}`'s unlock"),
            Owner::Bonfire { index } => format!("bonfire #{index}"),
            Owner::Shop { index } => format!("shop #{index}"),
            Owner::TrapDisarm { id } => format!("trap `{id}`'s disarm"),
            Owner::TimedGateDisarm { id } => format!("timed gate `{id}`'s disarm"),
        }
    }
}

/// Why a block is boxed, or that it is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Form {
    /// Pressed on its own: nothing is summoned.
    Own,
    /// Boxed because its press acts on its own: boxed outside the window.
    ActsOnOwn,
    /// Boxed because the game reports no empty-hand press of it: boxed always.
    Unobservable,
    /// Boxed because the press is a left-click, which the game reports on no
    /// block: boxed always.
    Struck,
}

impl Form {
    /// The words the ledger uses.
    pub fn word(self) -> &'static str {
        match self {
            Form::Own => "pressed on its own",
            Form::ActsOnOwn => "boxed: acts on its own",
            Form::Unobservable => "boxed: unobservable",
            Form::Struck => "boxed: struck",
        }
    }
}

/// One of the three states a press can be in (spec-0117 §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    /// Not open and not settled.
    Shut,
    /// The press acts.
    Open,
    /// The act is done.
    Settled,
}

impl State {
    /// The value the owner's state holder (`#pst_<owner>`) carries.
    pub fn value(self) -> u8 {
        match self {
            State::Shut => 0,
            State::Open => 1,
            State::Settled => 2,
        }
    }

    /// The state an answer of `a` is given in.
    pub fn of_answer(a: AnswerState) -> State {
        match a {
            AnswerState::NotYet => State::Shut,
            AnswerState::MissingItem => State::Open,
            AnswerState::Done => State::Settled,
        }
    }
}

/// **One interaction point, placed** (spec-0117 §3a).
#[derive(Clone, Debug)]
pub struct PressPlan {
    /// What it is.
    pub owner: Owner,
    /// The validation-tier kind (the states' authority).
    pub kind: PressKind,
    /// The entity tag its click box wears (`dw_i_<obj>`, `dw_trig_<id>`,
    /// `dw_sc_<id>`, `dw_bonfire_<i>`, `dw_shop_<i>`, `dw_trapdis_<trap>`,
    /// `dw_tgdis_<gate>`, `dw_gate_<safe>`).
    pub tag: String,
    /// The anchor its cell resolves from.
    pub anchor: String,
    /// The cell its block stands in (a door's lower half).
    pub cell: [i32; 3],
    /// The block as placed, namespaced (a door's lower half).
    pub block: String,
    /// How it is pressed.
    pub form: Form,
    /// The block's box stays through the open window although it acts on its
    /// own: an `interact` whose open state has a refusal inside it
    /// (`requires_item`), which the block's own act would bypass.
    pub retained: bool,
    /// `release`.
    pub release: bool,
    /// Whether the compiler places the block (every pressable but a gate's own
    /// answer, whose block is the gate's).
    pub places_block: bool,
    /// The press has a shut state.
    pub shut: bool,
    /// The press has a settled state.
    pub settled: bool,
    /// The press has a `missing_item` state: the item it needs.
    pub requires_item: Option<String>,
    /// The declared answers.
    pub answers: Option<Answers>,
    /// The declared highlight.
    pub highlight: Option<Highlight>,
    /// The fitted click box, when the block is boxed in some state.
    pub click_box: Option<crate::compiler::clickbox::ClickBox>,
}

impl PressPlan {
    /// The cells the block stands in: one, or a door's two.
    pub fn cells(&self) -> Vec<[i32; 3]> {
        if is_door(&self.block) {
            vec![self.cell, [self.cell[0], self.cell[1] + 1, self.cell[2]]]
        } else {
            vec![self.cell]
        }
    }

    /// The `setblock` lines that place the block: the state itself, and for a
    /// door its upper half (`half=upper`, same `facing`, `hinge` and `open`).
    pub fn setblocks(&self) -> Vec<String> {
        if !self.places_block {
            return Vec::new();
        }
        let [x, y, z] = self.cell;
        let mut v = vec![format!("setblock {x} {y} {z} {}", self.block)];
        if is_door(&self.block) {
            v.push(format!(
                "setblock {x} {} {z} {}",
                y + 1,
                upper_half(&self.block)
            ));
        }
        v
    }

    /// Whether the click box stands in `state`.
    pub fn boxed_in(&self, state: State) -> bool {
        match self.form {
            Form::Own => false,
            Form::Unobservable | Form::Struck => true,
            Form::ActsOnOwn if self.retained => !(state == State::Settled && self.release),
            Form::ActsOnOwn => match state {
                State::Open => false,
                State::Shut => true,
                State::Settled => !self.release,
            },
        }
    }

    /// Whether the press in `state` is detected by the block's own report
    /// (`default_block_use` at the cell) rather than by the box.
    pub fn block_detects(&self, state: State) -> bool {
        !self.boxed_in(state)
    }

    /// Whether the box is reconciled against the press's state each tick: a
    /// boxed trigger whose block acts on its own. (An `interact`, a shortcut,
    /// a bonfire and a disarm move theirs at their own events.)
    pub fn reconciled(&self) -> bool {
        matches!(self.owner, Owner::Trigger { .. })
            && self.form == Form::ActsOnOwn
            && !self.retained
    }

    /// The state at `setup_finish`.
    pub fn initial(&self) -> State {
        if self.shut { State::Shut } else { State::Open }
    }

    /// The declared answers, in [`AnswerState::ALL`] order.
    pub fn declared(&self) -> Vec<(AnswerState, String)> {
        self.answers
            .as_ref()
            .map(|a| {
                a.declared()
                    .into_iter()
                    .map(|(s, l)| (s, l.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The box's `summon` carrying `tags` (the owner's tag first, then every
    /// rider's), under the fixture class, as a click box answers: `response:1b`.
    pub fn box_summon(&self, tags: &[String]) -> Option<String> {
        let b = self.click_box.as_ref()?;
        let list = tags
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(",");
        Some(b.summon(&format!(
            "response:1b,Invulnerable:1b,Tags:[\"dw_fixture\",{list}]"
        )))
    }
}

/// Whether a block is a door (one block in two cells).
pub fn is_door(block: &str) -> bool {
    delvewright_dsl::blockshape::bare_id(block).ends_with("_door")
}

/// A door half: the same state with `half` set to `half`, properties sorted.
fn with_half(block: &str, half: &str) -> String {
    let (name, props) = match block.split_once('[') {
        Some((n, rest)) => (n, rest.trim_end_matches(']')),
        None => (block, ""),
    };
    let mut kv: Vec<String> = props
        .split(',')
        .filter(|p| !p.is_empty() && !p.starts_with("half="))
        .map(str::to_string)
        .collect();
    kv.push(format!("half={half}"));
    kv.sort();
    format!("{name}[{}]", kv.join(","))
}

/// A door's upper half: the same state with `half=upper`.
pub fn upper_half(block: &str) -> String {
    with_half(block, "upper")
}

/// A door's lower half — `half=lower`, whether or not the author wrote it; any
/// other block unchanged.
fn lower_half(block: &str) -> String {
    if is_door(block) {
        with_half(block, "lower")
    } else {
        block.to_string()
    }
}

/// `minecraft:<id>` for a block written with or without its namespace.
fn namespaced(block: &str) -> String {
    if delvewright_dsl::blockshape::base_id(block).contains(':') {
        block.to_string()
    } else {
        format!("minecraft:{block}")
    }
}

/// The base id of a prop block, its blockstate suffix stripped — what a
/// `default_block_use` predicate names.
pub fn block_id(block: &str) -> &str {
    block.split('[').next().unwrap_or(block)
}

/// `DW1059` (d)'s message for a `use` or `strike` trigger anchored at a gate
/// **region**, or `None` when it is not: a region is many cells, and a press
/// lands on one real block. The one wording both the trigger-body ledger and
/// [`check`] refuse with.
pub fn region_anchor_refusal(plan: &Plan, t: &delvewright_dsl::EnvTrigger) -> Option<String> {
    if !matches!(t.on, TriggerOn::Use | TriggerOn::Strike) {
        return None;
    }
    let at = t.at_anchor()?;
    if point_of(plan, at).is_some() || gate_region(plan, at).is_none() {
        return None;
    }
    Some(format!(
        "trigger `{}` is anchored at `{at}`, a gate **region** (arm d): a region is many cells, \
         and a press lands on one real block. Prescription: anchor the trigger at a point anchor \
         on the door's own cell — its body is then the gate's block there under a click box \
         fitted to it — and give it `\"serves\": {{\"gate\": \"{at}\"}}` if it is the gate's \
         answer",
        t.id
    ))
}

/// The gate region an anchor resolves to, if it names one.
pub fn gate_region(plan: &Plan, anchor: &str) -> Option<([i32; 3], [i32; 3], String)> {
    plan.anchors.iter().find_map(|((_, n), r)| match r {
        ResolvedAnchor::Gate { from, to, block } if n == anchor => {
            Some((*from, *to, block.clone()))
        }
        _ => None,
    })
}

/// Whether `cell` lies in the inclusive region `(a, b)`.
pub fn in_region(cell: [i32; 3], (a, b): ([i32; 3], [i32; 3])) -> bool {
    (0..3).all(|i| a[i].min(b[i]) <= cell[i] && cell[i] <= a[i].max(b[i]))
}

/// Whether `cell` is face-adjacent to the inclusive region `(a, b)` without
/// lying in it.
pub fn face_adjacent(cell: [i32; 3], region: ([i32; 3], [i32; 3])) -> bool {
    !in_region(cell, region)
        && [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ]
        .iter()
        .any(|d| in_region([cell[0] + d[0], cell[1] + d[1], cell[2] + d[2]], region))
}

/// True when `anchor` is a planned NPC's stand anchor — the cell where that
/// NPC's interaction hitbox lives. A strike trigger there rides that hitbox.
pub(crate) fn npc_stands_at(plan: &Plan, anchor: &str) -> bool {
    plan.npcs.iter().any(|n| {
        plan.campaign.npcs.content.npcs.iter().any(|d| {
            d.id.as_str() == n.npc_id && d.anchor.as_str() == anchor && d.offset == [0, 0, 0]
        })
    })
}

/// The cell an anchor resolves to when it is a point anchor.
pub fn point_of(plan: &Plan, anchor: &str) -> Option<[i32; 3]> {
    plan.anchors.iter().find_map(|((_, n), r)| match r {
        ResolvedAnchor::Point { pos, .. } if n == anchor => Some(*pos),
        _ => None,
    })
}

/// The form a placed block is pressed in.
fn form_of(block: &str, only_open: bool, struck: bool) -> Option<Form> {
    if struck {
        return delvewright_dsl::outline_in_cell(block).then_some(Form::Struck);
    }
    Some(match delvewright_dsl::press_form(block, only_open)? {
        PressForm::Own => Form::Own,
        PressForm::ActsOnOwn => Form::ActsOnOwn,
        PressForm::Unobservable => Form::Unobservable,
    })
}

/// The inputs one placed pressable is built from.
struct Seat {
    owner: Owner,
    tag: String,
    anchor: String,
    cell: [i32; 3],
    block: String,
    struck: bool,
    places_block: bool,
    requires_item: Option<String>,
    highlight: Option<Highlight>,
}

/// **Every interaction point this build places** (spec-0117 §3a), in
/// [`delvewright_dsl::press_sites`] order, resolved to its cell, block, form and
/// box. A site whose anchor resolves to no point, or whose block is no body, is
/// absent here: validation (`DW1059`) or the build's press proof has refused
/// it.
///
/// Cached on the plan ([`Plan::presses`]); this is the derivation.
pub fn derive(plan: &Plan) -> Vec<PressPlan> {
    let c = plan.campaign;
    let quests = &c.quests.content;
    let mut out: Vec<PressPlan> = Vec::new();
    let bonfires: Vec<_> = plan.bonfires().collect();
    for site in delvewright_dsl::press_sites(c) {
        let seat: Option<Seat> = match site.kind {
            PressKind::Interact => {
                let found = quests.quests.iter().enumerate().find_map(|(qi, q)| {
                    q.objectives
                        .iter()
                        .enumerate()
                        .find(|(_, o)| o.id().as_str() == site.id)
                        .map(|(oi, o)| (qi, q, oi, o))
                });
                let Some((qi, q, oi, o)) = found else {
                    continue;
                };
                let Objective::Interact {
                    anchor,
                    requires_item,
                    highlight,
                    ..
                } = o
                else {
                    continue;
                };
                let cell = plan
                    .quest_area(q.id.as_str())
                    .and_then(|a| plan.point(a, anchor.as_str()))
                    .or_else(|| point_of(plan, anchor.as_str()));
                match (cell, site.prop) {
                    (Some(cell), Some(p)) => Some(Seat {
                        owner: Owner::Interact {
                            quest: qi,
                            objective: oi,
                            id: site.id.clone(),
                        },
                        tag: format!("dw_i_{}", safe_local(&site.id)),
                        anchor: anchor.as_str().to_string(),
                        cell,
                        block: p.block.clone(),
                        struck: false,
                        places_block: true,
                        requires_item: requires_item.clone(),
                        highlight: *highlight,
                    }),
                    _ => None,
                }
            }
            PressKind::Use | PressKind::Strike => {
                let Some(t) = quests.triggers.iter().find(|t| t.id.as_str() == site.id) else {
                    continue;
                };
                if !matches!(t.on, TriggerOn::Use | TriggerOn::Strike) {
                    continue;
                }
                let Some(at) = t.at_anchor() else { continue };
                if matches!(t.on, TriggerOn::Strike) && npc_stands_at(plan, at) {
                    continue;
                }
                let cell = point_of(plan, at);
                // A gate's own answer: its body is the gate's block at its
                // cell inside the region it serves.
                let gate_answer = match (&t.serves, cell) {
                    (Some(Serves::Gate(g)), Some(cell)) if t.prop.is_none() => {
                        gate_region(plan, g.as_str())
                            .filter(|(from, to, _)| in_region(cell, (*from, *to)))
                            .map(|(_, _, block)| (g, cell, block))
                    }
                    _ => None,
                };
                if let Some((g, cell, gate_block)) = gate_answer {
                    let block = plan
                        .shortcuts
                        .iter()
                        .find(|s| s.gate_anchor == g.as_str())
                        .map_or(gate_block, |s| s.gate_block.clone());
                    Some(Seat {
                        owner: Owner::GateAnswer {
                            id: site.id.clone(),
                            gate: g.as_str().to_string(),
                        },
                        tag: format!("dw_gate_{}", safe_local(g.as_str())),
                        anchor: at.to_string(),
                        cell,
                        block,
                        struck: false,
                        places_block: false,
                        requires_item: None,
                        highlight: t.highlight,
                    })
                } else {
                    match (cell, &t.prop) {
                        (Some(cell), Some(p)) => Some(Seat {
                            owner: Owner::Trigger {
                                id: site.id.clone(),
                            },
                            tag: format!("dw_trig_{}", safe_local(&site.id)),
                            anchor: at.to_string(),
                            cell,
                            block: p.block.clone(),
                            struck: matches!(t.on, TriggerOn::Strike),
                            places_block: true,
                            requires_item: None,
                            highlight: t.highlight,
                        }),
                        _ => None,
                    }
                }
            }
            PressKind::Step => continue,
            PressKind::Shortcut => {
                let Some(sc) = plan.shortcuts.iter().find(|s| s.id == site.id) else {
                    continue;
                };
                let decl = quests.shortcuts.iter().find(|s| s.id.as_str() == site.id);
                site.prop.map(|p| Seat {
                    owner: Owner::Shortcut {
                        id: site.id.clone(),
                    },
                    tag: format!("dw_sc_{}", sc.safe),
                    anchor: sc.unlock_anchor.clone(),
                    cell: sc.unlock,
                    block: p.block.clone(),
                    struck: false,
                    places_block: true,
                    requires_item: None,
                    highlight: decl.and_then(|d| d.highlight),
                })
            }
            PressKind::Bonfire => {
                // `press_sites` walks the bonfires in the effect walk's order;
                // match one checkpoint per site, by anchor, in that order.
                let taken = out
                    .iter()
                    .filter(|p| matches!(p.owner, Owner::Bonfire { .. }) && p.anchor == site.id)
                    .count();
                let Some(bf) = bonfires
                    .iter()
                    .filter(|b| b.anchor == site.id)
                    .nth(taken)
                    .copied()
                else {
                    continue;
                };
                site.prop.map(|p| Seat {
                    owner: Owner::Bonfire { index: bf.index },
                    tag: format!("dw_bonfire_{}", bf.index),
                    anchor: bf.anchor.clone(),
                    cell: bf.pos,
                    block: p.block.clone(),
                    struck: false,
                    places_block: true,
                    requires_item: None,
                    highlight: bonfire_highlight(c, &site.path),
                })
            }
            PressKind::Shop => {
                let Some((i, sh)) = quests
                    .shops
                    .iter()
                    .enumerate()
                    .find(|(_, s)| s.id.as_str() == site.id)
                else {
                    continue;
                };
                match (point_of(plan, sh.anchor.as_str()), site.prop) {
                    (Some(cell), Some(p)) => Some(Seat {
                        owner: Owner::Shop { index: i },
                        tag: format!("dw_shop_{i}"),
                        anchor: sh.anchor.as_str().to_string(),
                        cell,
                        block: p.block.clone(),
                        struck: false,
                        places_block: true,
                        requires_item: None,
                        highlight: sh.highlight,
                    }),
                    _ => None,
                }
            }
            PressKind::TrapDisarm => {
                let Some(t) = plan.traps.iter().find(|t| t.id == site.id) else {
                    continue;
                };
                let Some(d) = &t.disarm else { continue };
                let decl = quests
                    .traps
                    .iter()
                    .find(|x| x.id.as_str() == site.id)
                    .and_then(|x| x.disarm.as_ref());
                site.prop.map(|p| Seat {
                    owner: Owner::TrapDisarm {
                        id: site.id.clone(),
                    },
                    tag: format!("dw_trapdis_{}", t.safe),
                    anchor: d.via_anchor.clone(),
                    cell: d.via_cell,
                    block: p.block.clone(),
                    struck: false,
                    places_block: true,
                    requires_item: None,
                    highlight: decl.and_then(|x| x.highlight),
                })
            }
            PressKind::TimedGateDisarm => {
                let Some(g) = plan.timed_gates.iter().find(|g| g.id == site.id) else {
                    continue;
                };
                let Some(d) = &g.disarm else { continue };
                let decl = quests
                    .timed_gates
                    .iter()
                    .find(|x| x.id.as_str() == site.id)
                    .and_then(|x| x.disarm.as_ref());
                site.prop.map(|p| Seat {
                    owner: Owner::TimedGateDisarm {
                        id: site.id.clone(),
                    },
                    tag: format!("dw_tgdis_{}", g.safe),
                    anchor: d.via_anchor.clone(),
                    cell: d.via_cell,
                    block: p.block.clone(),
                    struck: false,
                    places_block: true,
                    requires_item: None,
                    highlight: decl.and_then(|x| x.highlight),
                })
            }
        };
        let Some(seat) = seat else { continue };
        let block = lower_half(&namespaced(&seat.block));
        let Some(form) = form_of(&block, site.only_open, seat.struck) else {
            continue;
        };
        let click_box = if form == Form::Own {
            None
        } else {
            match crate::compiler::clickbox::ClickBox::fitted_to(seat.cell, &block) {
                Some(b) => Some(b),
                None => continue,
            }
        };
        out.push(PressPlan {
            owner: seat.owner,
            kind: site.kind,
            tag: seat.tag,
            anchor: seat.anchor,
            cell: seat.cell,
            block,
            form,
            retained: form == Form::ActsOnOwn && seat.requires_item.is_some(),
            release: site.release,
            places_block: seat.places_block,
            // A shortcut's `not_yet` is its sealed door's, from the far side:
            // the unlock itself is open from load.
            shut: site.shut && site.kind != PressKind::Shortcut,
            settled: site.settled,
            requires_item: seat.requires_item,
            answers: site.answers.cloned(),
            highlight: seat.highlight,
            click_box,
        });
    }
    out
}

/// The `highlight` a bonfire effect at `path` declares.
fn bonfire_highlight(c: &delvewright_dsl::Campaign, path: &str) -> Option<Highlight> {
    let mut found = None;
    delvewright_dsl::for_each_campaign_effect(c, &mut |p, _site, eff| {
        if p == path
            && let delvewright_dsl::Verb::Bonfire { highlight, .. } = &eff.verb
        {
            found = *highlight;
        }
    });
    found
}

/// The press whose box wears `tag`, if one does.
pub fn press_by_tag<'p>(plan: &'p Plan, tag: &str) -> Option<&'p PressPlan> {
    plan.presses().iter().find(|p| p.tag == tag)
}

/// The press an authored trigger owns (its block and its box).
pub fn press_of_trigger<'p>(plan: &'p Plan, id: &str) -> Option<&'p PressPlan> {
    plan.presses().iter().find(|p| match &p.owner {
        Owner::Trigger { id: t } | Owner::GateAnswer { id: t, .. } => t == id,
        _ => false,
    })
}

/// The press an `interact` objective owns.
pub fn press_of_objective<'p>(plan: &'p Plan, id: &str) -> Option<&'p PressPlan> {
    plan.presses()
        .iter()
        .find(|p| matches!(&p.owner, Owner::Interact { id: o, .. } if o == id))
}

/// The gate's own answer press over shortcut `id`'s sealed door, if one stands.
pub fn shortcut_door<'p>(plan: &'p Plan, id: &str) -> Option<&'p PressPlan> {
    let gate = &plan.shortcuts.iter().find(|s| s.id == id)?.gate_anchor;
    plan.presses()
        .iter()
        .find(|p| matches!(&p.owner, Owner::GateAnswer { gate: g, .. } if g == gate))
}

/// **One declared answer** (spec-0117 §5.2): its owner, its state and its line,
/// lowered to a presser trigger by [`AnswerPlan::trigger`].
#[derive(Clone, Debug)]
pub struct AnswerPlan {
    /// The owner's token (`obj-pry`).
    pub owner_token: String,
    /// The owner's safe name (`obj_pry`): the state holder `#pst_<it>` and the
    /// trace holder `#ans_<it>`.
    pub owner_safe: String,
    /// The owner's index in [`Plan::presses`], when it is a placed pressable;
    /// `None` for a trigger whose body is a creature (a `strike-npc`, a
    /// `strike-assembly`, a strike at an NPC's stand), a step, or a shortcut's
    /// far-side line.
    pub press: Option<usize>,
    /// The owner itself.
    pub owner: Owner,
    /// Which state it answers.
    pub state: AnswerState,
    /// The campaign's own line.
    pub text: String,
    /// The event it is pressed by — the owner's own.
    pub on: TriggerOn,
    /// The anchor it watches, when the owner watches one.
    pub at: Option<String>,
    /// For a shortcut's `not_yet`: the shortcut, whose sealed door's box this
    /// answer rides, said to a presser off the sealed side.
    pub far_side_of: Option<String>,
}

impl AnswerPlan {
    /// The synthesized trigger's id: `trigger/dw-answer-<owner>-<state>`.
    pub fn id(&self) -> String {
        format!(
            "trigger/dw-answer-{}-{}",
            self.owner_token,
            self.state.token()
        )
    }

    /// The trigger this answer lowers to: a repeatable presser press of the
    /// owner's own kind, one `narrate` on the action bar. Its gate is the
    /// owner's state, read by `press_<id>` from `#pst_<owner>`, never typed
    /// here.
    pub fn trigger(&self) -> EnvTrigger {
        EnvTrigger {
            id: delvewright_dsl::TriggerId(self.id()),
            at: self.at.clone().map(delvewright_dsl::AnchorId),
            prop: None,
            answers: None,
            release: false,
            serves: None,
            highlight: None,
            on: self.on.clone(),
            requires_flags: Vec::new(),
            forbids_flags: Vec::new(),
            requires_state: Vec::new(),
            once: false,
            audience: delvewright_dsl::TriggerAudience::Presser,
            effects: vec![
                delvewright_dsl::Verb::Narrate(delvewright_dsl::Narration {
                    text: self.text.clone(),
                    style: Some(delvewright_dsl::NarrateStyle::Actionbar),
                    sound: None,
                })
                .into(),
            ],
        }
    }
}

/// **Every declared answer this build emits** (spec-0117 §5.2), owner by owner
/// in [`Plan::presses`] order then the creature-bodied triggers in declaration
/// order, states in [`AnswerState::ALL`] order. Deterministic.
pub fn derive_answers(plan: &Plan) -> Vec<AnswerPlan> {
    let quests = &plan.campaign.quests.content;
    let mut out = Vec::new();
    for (pi, p) in plan.presses().iter().enumerate() {
        let on = match &p.owner {
            Owner::Trigger { id } | Owner::GateAnswer { id, .. } => quests
                .triggers
                .iter()
                .find(|t| t.id.as_str() == id)
                .map_or(TriggerOn::Use, |t| t.on.clone()),
            _ => TriggerOn::Use,
        };
        for (state, text) in p.declared() {
            // A shortcut's `not_yet` is the sealed door's line from the far
            // side: it rides the door's own box, not the unlock.
            let far_side_of = match (&p.owner, state) {
                (Owner::Shortcut { id }, AnswerState::NotYet) => Some(id.clone()),
                _ => None,
            };
            let at = match &far_side_of {
                Some(id) => shortcut_door(plan, id).map(|q| q.anchor.clone()),
                None => Some(p.anchor.clone()),
            };
            out.push(AnswerPlan {
                owner_token: p.owner.token(),
                owner_safe: p.owner.safe(),
                press: if far_side_of.is_some() {
                    None
                } else {
                    Some(pi)
                },
                owner: p.owner.clone(),
                state,
                text,
                on: on.clone(),
                at,
                far_side_of,
            });
        }
    }
    // The triggers with no block of their own: a strike on a creature or an
    // assembly, a strike at an NPC's stand, a step. Their answers ride the body
    // the trigger rides, on the trigger's own event.
    for t in &quests.triggers {
        let rides = match &t.on {
            TriggerOn::StrikeNpc { .. } | TriggerOn::StrikeAssembly { .. } | TriggerOn::Step => {
                true
            }
            TriggerOn::Strike => t.at_anchor().is_some_and(|a| npc_stands_at(plan, a)),
            _ => false,
        };
        if !rides {
            continue;
        }
        let Some(a) = &t.answers else { continue };
        let owner = Owner::Trigger {
            id: t.id.as_str().to_string(),
        };
        for (state, text) in a.declared() {
            out.push(AnswerPlan {
                owner_token: owner.token(),
                owner_safe: owner.safe(),
                press: None,
                owner: owner.clone(),
                state,
                text: text.clone(),
                on: t.on.clone(),
                at: t.at.as_ref().map(|a| a.as_str().to_string()),
                far_side_of: None,
            });
        }
    }
    out
}

/// The answer a synthesized trigger id is, if it is one.
pub fn answer_of<'p>(plan: &'p Plan, trigger_id: &str) -> Option<&'p AnswerPlan> {
    plan.answers().iter().find(|a| a.id() == trigger_id)
}

/// The tags a press's box carries: its own first, then every trigger that
/// reads the box — the authored trigger it is, every answer that rides it, and,
/// on a shortcut's door, the shortcut's far-side line.
pub fn box_tags(plan: &Plan, p: &PressPlan) -> Vec<String> {
    let mut tags = vec![p.tag.clone()];
    let mut add = |t: String| {
        if !tags.contains(&t) {
            tags.push(t);
        }
    };
    if let Owner::Trigger { id } | Owner::GateAnswer { id, .. } = &p.owner {
        add(format!("dw_trig_{}", safe_local(id)));
    }
    // Every other gate answer at the same cell shares the one box (one cell,
    // one box: `DW0422`).
    for q in plan.presses() {
        if q.tag == p.tag
            && q.cell == p.cell
            && let Owner::Trigger { id } | Owner::GateAnswer { id, .. } = &q.owner
        {
            add(format!("dw_trig_{}", safe_local(id)));
        }
    }
    for a in plan.answers() {
        let rides = match (&a.press, &a.far_side_of) {
            (Some(i), _) => plan
                .presses()
                .get(*i)
                .is_some_and(|q| q.tag == p.tag && q.cell == p.cell),
            (None, Some(sc)) => shortcut_door(plan, sc).is_some_and(|q| q.tag == p.tag),
            _ => false,
        };
        if rides {
            add(format!("dw_trig_{}", safe_local(&a.id())));
        }
    }
    tags
}

/// What a click on a trigger lands on — the body its dispatch reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    /// A box carries this trigger's tag: a click box over a block, a gate's own
    /// answer box, an NPC's hitbox, an actor's answer box. Carries the box's tag
    /// and a human name for diagnostics.
    Rides {
        /// The tag of the box the trigger rides.
        tag: String,
        /// What owns that box.
        owner: &'static str,
    },
    /// **The real block the player presses, reporting its own press**
    /// (`default_block_use` at `cell`): no box stands in the state this trigger
    /// acts in.
    Block {
        /// The cell the block stands in.
        cell: [i32; 3],
        /// The block.
        block: String,
    },
    /// Nothing here resolves — `DW0426`.
    Nothing,
}

/// The body a trigger the build emits is dispatched from.
///
/// An authored `use` or `strike` at a point is its press's block: the block
/// itself where the open state has no box, the box where it has one. An answer
/// is its owner's body in the state it answers. A strike on a creature rides
/// the creature (decided by the caller that knows the kind); a body's answer
/// (spec-0119) rides the box its body owns.
pub fn trigger_body(plan: &Plan, t: &EnvTrigger) -> Body {
    if let Some(a) = crate::compiler::bodies::answer_of(plan.campaign, t.id.as_str()) {
        let (tag, owner) = a.carrier(plan);
        return Body::Rides { tag, owner };
    }
    if let Some(a) = answer_of(plan, t.id.as_str()) {
        if let Some(sc) = &a.far_side_of {
            return match shortcut_door(plan, sc) {
                Some(q) => Body::Rides {
                    tag: q.tag.clone(),
                    owner: "shortcut door",
                },
                None => Body::Nothing,
            };
        }
        let Some(p) = a.press.and_then(|i| plan.presses().get(i)) else {
            // A creature-bodied trigger's answer: the trigger's own body.
            return Body::Nothing;
        };
        let state = State::of_answer(a.state);
        return if p.block_detects(state) {
            Body::Block {
                cell: p.cell,
                block: p.block.clone(),
            }
        } else {
            Body::Rides {
                tag: p.tag.clone(),
                owner: "click box",
            }
        };
    }
    if let Some(p) = press_of_trigger(plan, t.id.as_str()) {
        return if p.block_detects(State::Open) {
            Body::Block {
                cell: p.cell,
                block: p.block.clone(),
            }
        } else {
            Body::Rides {
                tag: p.tag.clone(),
                owner: match p.owner {
                    Owner::GateAnswer { .. } => "gate's own box",
                    _ => "click box",
                },
            }
        };
    }
    Body::Nothing
}

/// **The blocks every pressable places at load**, as `(owner token,
/// placement)`, a door contributing both halves, in [`Plan::presses`] order.
/// `setup_finish` writes exactly these and the world as shipped
/// ([`crate::compiler::assembled::shipped_blocks`]) lays exactly these, so the
/// written world and the server's world cannot disagree about a prop.
pub fn props(plan: &Plan) -> Vec<(String, crate::compiler::light::Placement)> {
    let mut out = Vec::new();
    for p in plan.presses() {
        if !p.places_block {
            continue;
        }
        out.push((
            p.owner.token(),
            crate::compiler::light::Placement {
                pos: p.cell,
                block: p.block.clone(),
            },
        ));
        if is_door(&p.block) {
            out.push((
                p.owner.token(),
                crate::compiler::light::Placement {
                    pos: [p.cell[0], p.cell[1] + 1, p.cell[2]],
                    block: upper_half(&p.block),
                },
            ));
        }
    }
    out
}

/// What a click at a trigger lands on, in one line of prose.
pub fn describe(body: &Body) -> String {
    match body {
        Body::Rides { owner, tag } => format!("rides the {owner} (`{tag}`)"),
        Body::Block { cell, block } => {
            format!("the block `{block}` at {cell:?}, whose own press vanilla reports")
        }
        Body::Nothing => "nothing — DW0426".to_string(),
    }
}

/// **The press-body ledger** (`validation/press-bodies.json`, spec-0117 §5.1):
/// every interaction point's block, its form, and for a boxed one its fitted
/// box; every click trigger's resolved body. The binding count is the
/// population.
#[derive(Clone, Debug, Default)]
pub struct PressLedger {
    /// One row per placed interaction point.
    pub bodies: Vec<serde_json::Value>,
    /// `(trigger id, click kind, anchor, what the click lands on)`, in emitted
    /// order.
    pub presses: Vec<(String, String, String, String)>,
}

impl PressLedger {
    /// The ledger rows of `plan`'s pressables.
    pub fn bodies_of(plan: &Plan) -> Vec<serde_json::Value> {
        plan.presses()
            .iter()
            .map(|p| {
                let mut row = serde_json::json!({
                    "owner": p.owner.token(),
                    "what": p.owner.word(),
                    "anchor": p.anchor,
                    "cell": p.cell,
                    "block": p.block,
                    "form": p.form.word(),
                    "release": p.release,
                    "box_tag": p.tag,
                    "boxed_in": {
                        "shut": p.shut && p.boxed_in(State::Shut),
                        "open": p.boxed_in(State::Open),
                        "settled": p.settled && p.boxed_in(State::Settled),
                    },
                    "answers": p.declared().iter().map(|(s, _)| s.field()).collect::<Vec<_>>(),
                });
                if let Some(b) = &p.click_box {
                    row["box"] = serde_json::json!({
                        "at": b.xyz(),
                        "nbt": b.size_nbt(),
                        "width": b.width_text(),
                        "height": b.height_text(),
                    });
                }
                row
            })
            .collect()
    }

    /// Record one resolved click.
    pub fn push(&mut self, trigger: &str, kind: &str, anchor: &str, body: &str) {
        self.presses.push((
            trigger.to_string(),
            kind.to_string(),
            anchor.to_string(),
            body.to_string(),
        ));
    }

    /// The ledger as the `validation/press-bodies.json` artifact.
    pub fn to_json(&self) -> serde_json::Value {
        let n = self.presses.len();
        let mut o = serde_json::json!({
            "code": DW_TRIGGER_UNPRESSABLE,
            "bodies": self.bodies,
            "pressables": self.bodies.len(),
            "presses": self
                .presses
                .iter()
                .map(|(id, kind, anchor, body)| serde_json::json!({
                    "trigger": id,
                    "click": kind,
                    "anchor": anchor,
                    "body": body,
                }))
                .collect::<Vec<_>>(),
            "examined": n,
            "unbound": n == 0 && self.bodies.is_empty(),
        });
        if n == 0 && self.bodies.is_empty() {
            o["reason"] = serde_json::json!(
                "this campaign arms no interaction point and no click trigger at all, so \
                 nothing here can be pressed and DW0426 had nothing to resolve a body for"
            );
        }
        o
    }
}

/// The counts of the placed pressables, by form.
pub fn form_counts(plan: &Plan) -> BTreeMap<Form, usize> {
    let mut m = BTreeMap::new();
    for p in plan.presses() {
        *m.entry(p.form).or_insert(0) += 1;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_door_is_named_by_its_lower_half_and_placed_whole() {
        assert_eq!(
            lower_half("minecraft:spruce_door[facing=south,hinge=left,open=false]"),
            "minecraft:spruce_door[facing=south,half=lower,hinge=left,open=false]"
        );
        assert_eq!(
            upper_half("minecraft:spruce_door[facing=south,half=lower,hinge=left,open=false]"),
            "minecraft:spruce_door[facing=south,half=upper,hinge=left,open=false]"
        );
        assert!(is_door("minecraft:iron_door"));
        assert!(!is_door("minecraft:oak_trapdoor"));
    }

    #[test]
    fn face_adjacency_is_off_the_region_and_touching_it() {
        let r = ([4, 65, 6], [5, 67, 6]);
        assert!(face_adjacent([4, 65, 5], r));
        assert!(!face_adjacent([4, 65, 6], r));
        assert!(!face_adjacent([3, 65, 5], r));
    }
}

// ---------------------------------------------------------------------------
// The build's half of the press proof: the arms that need the placed world
// ---------------------------------------------------------------------------

/// What the build's press proof examined, stated whichever way it went.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PressProof {
    /// Interaction points placed, by form.
    pub forms: BTreeMap<Form, usize>,
    /// Sealed gates (`close-gate` targets and shortcut gates) judged by
    /// `DW0429`.
    pub gates: usize,
    /// Of their answers, the gate's own (a box over its block).
    pub own_answers: usize,
    /// Of their answers, knockers beside the region.
    pub knockers: usize,
    /// Answer triggers synthesized (`trigger/dw-answer-…`).
    pub answers: usize,
    /// Reconciliation pairs the tick carries (a boxed trigger's box made its
    /// state).
    pub reconciled: usize,
}

impl PressProof {
    /// The line a build prints.
    pub fn line(&self) -> String {
        let n = |f: Form| self.forms.get(&f).copied().unwrap_or(0);
        format!(
            "press proof (DW0429/DW1059/DW1052/DW0896): {} pressable(s) placed — {} pressed on \
             their own, {} boxed acting on their own, {} boxed unobservable, {} boxed struck; {} \
             sealed gate(s) answered ({} by their own block, {} by a knocker); {} answer \
             trigger(s) synthesized; {} reconciliation pair(s) in tick",
            self.forms.values().sum::<usize>(),
            n(Form::Own),
            n(Form::ActsOnOwn),
            n(Form::Unobservable),
            n(Form::Struck),
            self.gates,
            self.own_answers,
            self.knockers,
            self.answers,
            self.reconciled
        )
    }

    /// The record `validation/press-proof.json` carries.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "pressables": self.forms.values().sum::<usize>(),
            "forms": self.forms.iter().map(|(k, v)| (k.word().to_string(), serde_json::json!(v))).collect::<serde_json::Map<String, serde_json::Value>>(),
            "gates": self.gates,
            "own_answers": self.own_answers,
            "knockers": self.knockers,
            "answers": self.answers,
            "reconciled": self.reconciled,
        })
    }
}

/// **The press proof the placed world decides** (spec-0117 §6): a trigger
/// anchored at a gate **region** (`DW1059` (d)); a knocker with no block of its
/// own (`DW1059` (a)); a `prop` on a gate's own answer (`DW1052`); every sealed
/// gate answered (`DW0429`), in either form, a shortcut's by its own leaf from
/// the sealed side; and two pressables on one cell agreeing about the block,
/// with at most one of them boxed (`DW0896`, re-stated over blocks). `blocks`
/// is the assembled world, read for the block a shortcut's door holds at an
/// answer's cell.
pub fn check(
    plan: &Plan,
    blocks: &crate::compiler::blockstate::BlockMap,
) -> (PressProof, Result<(), crate::compiler::failure::Failure>) {
    use crate::compiler::failure::Failure;
    use delvewright_dsl::{PRESS_ANSWER_INERT, PRESS_TARGET_UNREAL};
    let c = plan.campaign;
    let quests = &c.quests.content;
    let mut b = PressProof {
        forms: form_counts(plan),
        answers: plan.answers().len(),
        reconciled: plan.presses().iter().filter(|p| p.reconciled()).count(),
        ..PressProof::default()
    };
    let fail = |code, message: String| Err(Failure::new(code, message));
    // --- DW1059 (d), the knocker's block, the gate answer's prop ---
    for t in &quests.triggers {
        if !matches!(t.on, TriggerOn::Use | TriggerOn::Strike) {
            continue;
        }
        let Some(at) = t.at_anchor() else { continue };
        if matches!(t.on, TriggerOn::Strike) && npc_stands_at(plan, at) {
            continue;
        }
        if let Some(message) = region_anchor_refusal(plan, t) {
            return (b, fail(PRESS_TARGET_UNREAL, message));
        }
        let Some(Serves::Gate(g)) = &t.serves else {
            continue;
        };
        let (Some(cell), Some((from, to, _))) = (point_of(plan, at), gate_region(plan, g.as_str()))
        else {
            continue;
        };
        let inside = in_region(cell, (from, to));
        if inside && t.prop.is_some() {
            return (
                b,
                fail(
                    PRESS_ANSWER_INERT,
                    format!(
                        "trigger `{}` is anchored at {cell:?}, inside the gate `{g}` it serves, and \
                         declares a `prop`: the gate names its own block at that cell, so a second \
                         declaration of one cell's block is a second authority. Prescription: drop \
                         `prop` — the gate's block is the body",
                        t.id
                    ),
                ),
            );
        }
        if !inside && t.prop.is_none() {
            return (
                b,
                fail(
                    PRESS_TARGET_UNREAL,
                    format!(
                        "trigger `{}` serves the gate `{g}` from {cell:?}, outside it, and names no \
                         block: a knocker is a real block beside the gate (arm a). Prescription: \
                         give it `\"prop\": {{\"block\": \"minecraft:bell[attachment=floor]\"}}` (or a \
                         button), or anchor it at the door's own cell to answer through the gate's \
                         block",
                        t.id
                    ),
                ),
            );
        }
    }
    // --- DW0429: every sealed gate answered ---
    let mut gates: Vec<(String, bool)> = Vec::new();
    for s in &plan.seal_hints {
        gates.push((s.anchor.clone(), false));
    }
    for sc in &plan.shortcuts {
        if !gates.iter().any(|(g, _)| g == &sc.gate_anchor) {
            gates.push((sc.gate_anchor.clone(), true));
        }
    }
    b.gates = gates.len();
    for (gate, shortcut) in &gates {
        let Some((from, to, _)) = gate_region(plan, gate) else {
            continue;
        };
        let answers: Vec<&EnvTrigger> = quests
            .triggers
            .iter()
            .filter(|t| {
                matches!(t.on, TriggerOn::Use)
                    && matches!(&t.serves, Some(Serves::Gate(g)) if g.as_str() == gate)
            })
            .collect();
        let mut own = 0usize;
        let mut knock = 0usize;
        for t in &answers {
            let Some(cell) = t.at_anchor().and_then(|a| point_of(plan, a)) else {
                continue;
            };
            if in_region(cell, (from, to)) {
                // The cell must hold a block while sealed: a `close-gate` fills
                // its region with the gate's block; a shortcut's door is the
                // prefab's.
                if *shortcut {
                    // The shortcut's gate is sealed from load: the shipped world
                    // holds the gate's declared block over its whole region (the
                    // world-load seal, `assembled::shipped_blocks`), whatever the
                    // template's bytes say there.
                    let sealed_with = gate_region(plan, gate)
                        .map(|(_, _, b)| b)
                        .filter(|b| !delvewright_dsl::blockshape::is_air(b));
                    let held = blocks
                        .get(&cell)
                        .map(|s| s.as_str().to_string())
                        .filter(|s| !delvewright_dsl::blockshape::is_air(s))
                        .or(sealed_with);
                    if held.is_none() {
                        return (
                            b,
                            fail(
                                SEALED_GATE_UNANSWERED,
                                format!(
                                    "trigger `{}` answers the sealed gate `{gate}` (cells \
                                     {from:?}..{to:?}) from {cell:?}, a cell of the door the \
                                     prefab leaves as air: there is no block there to press. \
                                     Prescription: anchor the answer at a cell of the door that \
                                     holds its block",
                                    t.id
                                ),
                            ),
                        );
                    }
                    if !t.addresses_presser() {
                        return (
                            b,
                            fail(
                                SEALED_GATE_UNANSWERED,
                                format!(
                                    "trigger `{}` is the shortcut gate `{gate}`'s own answer and \
                                     addresses the party: the door is pressed from both sides, \
                                     and only the presser's position says which side the hand is \
                                     on. Prescription: `\"audience\": \"presser\"`",
                                    t.id
                                ),
                            ),
                        );
                    }
                }
                own += 1;
            } else if face_adjacent(cell, (from, to))
                && (!*shortcut || on_sealed_side(plan, gate, cell))
            {
                knock += 1;
            } else {
                return (
                    b,
                    fail(
                        SEALED_GATE_UNANSWERED,
                        format!(
                            "trigger `{}` serves the gate `{gate}` from {cell:?}, which is {}: a \
                             gate's answer is its own block inside the region, or a knocker \
                             face-adjacent to it{}. Prescription: anchor it at the door's own \
                             cell, or beside the door on the side that owes the line",
                            t.id,
                            if *shortcut && face_adjacent(cell, (from, to)) {
                                "beside the shortcut's door on its far side"
                            } else {
                                "neither inside the region nor face-adjacent to it"
                            },
                            if *shortcut {
                                " on the sealed side, where the party meets the barred door"
                            } else {
                                ""
                            }
                        ),
                    ),
                );
            }
        }
        b.own_answers += own;
        b.knockers += knock;
        if own + knock == 0 {
            let what = if *shortcut {
                format!(
                    "the shortcut gate `{gate}` is barred from world-load and the party is led to \
                     its wrong side first"
                )
            } else {
                format!("a `close-gate` seals `{gate}`, a wall the party walks back to")
            };
            return (
                b,
                fail(
                    SEALED_GATE_UNANSWERED,
                    format!(
                        "{what}, and nothing answers a press on it: no `use` trigger `serves` it. \
                         A push on a sealed door is otherwise silent, and the compiler does not \
                         word it for you. Prescription: anchor a trigger at a point on the door's \
                         own cell — `{{\"id\": \"trigger/<name>\", \"at\": \"<the door cell's \
                         anchor>\", \"on\": {{\"on\": \"use\"}}, \"once\": false, \"audience\": \
                         \"presser\", \"serves\": {{\"gate\": \"{gate}\"}}, \"effects\": [{{\"type\": \
                         \"narrate\", \"style\": \"actionbar\", \"text\": \"<what the door \
                         says>\"}}]}}` — whose body is the gate's own block{}",
                        if *shortcut {
                            ""
                        } else {
                            "; or, where nothing reads as the way through, a knocker (a bell or a \
                             button with its `prop`) face-adjacent to the region"
                        }
                    ),
                ),
            );
        }
    }
    // --- DW0896, over blocks: one cell, one block, at most one box ---
    let mut at_cell: BTreeMap<[i32; 3], &PressPlan> = BTreeMap::new();
    for p in plan.presses() {
        for cell in p.cells() {
            if let Some(q) = at_cell.get(&cell) {
                if matches!(
                    (&q.owner, &p.owner),
                    (Owner::GateAnswer { .. }, Owner::GateAnswer { .. })
                ) {
                    continue;
                }
                if q.block != p.block || (q.form != Form::Own && p.form != Form::Own) {
                    return (
                        b,
                        fail(
                            crate::compiler::cohabit::DW_ONE_MARK_TWO_BODIES,
                            format!(
                                "{} and {} both stand at {cell:?}{}. One cell holds one block, \
                                 and a boxed block one click box: two boxes over it are an exact \
                                 ray-pick tie. Prescription: move one of them to a cell of its \
                                 own, or make both the same block pressed on its own (a bell, a \
                                 button)",
                                q.owner.word(),
                                p.owner.word(),
                                if q.block != p.block {
                                    format!(" as `{}` and `{}`", q.block, p.block)
                                } else {
                                    " and both are boxed".to_string()
                                }
                            ),
                        ),
                    );
                }
            } else {
                at_cell.insert(cell, p);
            }
        }
    }
    (b, Ok(()))
}

/// Whether `cell` lies on shortcut gate `gate`'s sealed side
/// ([`crate::compiler::wrongside`]): beyond the door's slab on the side the
/// unlock is not.
fn on_sealed_side(plan: &Plan, gate: &str, cell: [i32; 3]) -> bool {
    plan.shortcuts
        .iter()
        .find(|s| s.gate_anchor == gate)
        .and_then(|s| s.sealed_side.as_ref())
        .is_some_and(|sd| {
            if sd.dir < 0 {
                cell[sd.axis] < sd.gate_lo[sd.axis]
            } else {
                cell[sd.axis] > sd.gate_hi[sd.axis]
            }
        })
}

/// [`check`]'s `DW0429` code, re-exported where its callers look.
pub use crate::compiler::plan::SEALED_GATE_UNANSWERED;
