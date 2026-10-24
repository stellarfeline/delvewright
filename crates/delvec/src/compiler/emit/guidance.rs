//! Guidance a player can follow (spec-0104 §6–§8): the compass each player
//! aims at an open objective, the proximity highlight, and the journal.
//!
//! One derivation ([`Guide`]) is read by every emitter here, so the compass,
//! the journal and the highlight can never disagree about which objectives are
//! tracked, which are announced, or what a highlight lights.
//!
//! **When anything is rewritten.** The tick keeps one state per guided
//! objective — `0` closed, `1` open (its `told_guard` holds), `2`
//! complete — and moves the campaign's guidance epoch when any of them changes:
//! that is exactly an objective or a quest activating or completing, or a gate
//! opening or closing. A classed player whose last refresh is older than the
//! epoch is refreshed — the compass re-aimed and the journal rewritten — so a
//! join, a respawn and a dropped item reach the same function, and nothing is
//! rewritten on a tick where nothing moved.

use super::*;

/// The dummy objective every guidance holder lives on.
pub(super) const GUIDE: &str = "dw.guide";
/// Per player: the ordinal of the tracked objective followed (`0` none).
pub(super) const TRACK: &str = "dw.track";
/// Per player: the ordinal last told in chat (`-1` forces the line).
const TOLD: &str = "dw.told";
/// Per player: the epoch last refreshed at.
const SEEN: &str = "dw.gseen";
/// Per player: the press latch — set while the compass is held in use, run
/// down by the tick once it is not, so one press opens the menu once.
const USE_LATCH: &str = "dw.cuse";
/// Per player, a `trigger`: the ordinal a menu button picks.
pub(super) const PICK: &str = "dw.cpick";
/// Per player: scratch for the next ordinal a cycle moves to.
const NEXT: &str = "dw.tnext";
/// Per player: `1` while the followed objective is a `talk-to` or a `kill`,
/// whose target moves.
const ON_BODY: &str = "dw.tbody";
/// Per player: deaths since the last refresh (`deathCount`).
const DEATHS: &str = "dw.gdeath";
/// The tag a player carries once their guidance entries are seeded.
const SEEDED_TAG: &str = "dw_guide_seeded";
/// The campaign's guidance epoch holder.
const EPOCH: &str = "#epoch";
/// The twenty-tick clock that re-aims a compass following a body or a wave.
const CLOCK: &str = "#gclock";
/// The engine compass's `custom_data`.
pub(super) const COMPASS_DATA: &str = "{dw_compass:1b}";
/// The journal's `custom_data`.
pub(super) const JOURNAL_DATA: &str = "{dw_journal:1b}";
/// The highlight team.
pub(super) const HL_TEAM: &str = "dw_hl";
/// The highlight colour: the team's `gold`, and the same colour on a display
/// (`0xFFAA00`, `ChatFormatting.GOLD`).
const HL_COLOR: &str = "gold";
const HL_RGB: u32 = 0xFFAA00;
/// The advancement that sees the compass used (`minecraft:using_item`).
pub(super) const COMPASS_USE_ADVANCEMENT: &str = "compass_use";

/// The item predicate of the engine's compass.
pub(super) fn compass_item() -> String {
    format!("minecraft:compass[custom_data~{COMPASS_DATA}]")
}

/// The item predicate of the journal.
pub(super) fn journal_item() -> String {
    format!("minecraft:written_book[custom_data~{JOURNAL_DATA}]")
}

/// One objective the guidance machinery watches: announced (the journal lists
/// it), tracked (the compass follows it), or highlighted.
#[derive(Clone, Debug)]
pub(super) struct Watched {
    /// Quest index and objective index in stage 5.
    pub quest: usize,
    pub objective: usize,
    /// Objective id.
    pub id: String,
    /// `safe_local` of the id.
    pub safe: String,
    /// Its `told_guard` — the window the party is told of it in — a run of
    /// ` if …`/` unless …` clauses.
    pub guard: String,
    /// Announced: listed in the journal.
    pub announced: bool,
    /// Tracked: its ordinal on the compass, from 1, in document order.
    pub ordinal: Option<usize>,
    /// Where the compass points: a cell, a body's tag (a `talk-to`'s NPC), or
    /// a wave (a `kill`'s).
    pub target: Option<Target>,
}

/// Where the compass points for one objective (§6.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Target {
    /// A fixed cell.
    Cell([i32; 3]),
    /// A body's entity tag; read live.
    Body(String),
    /// A wave: the living body of its tag nearest the player holding the
    /// compass, read live; its anchor while none is alive.
    Wave {
        tag: String,
        anchor: Option<[i32; 3]>,
    },
}

/// The tag a wave's nearest living body carries for the length of one aim.
const WAVE_AIM_TAG: &str = "dw_caim";

/// What one highlight lights (§7.2).
#[derive(Clone, Debug)]
pub(super) enum HlBody {
    /// Block displays: one per cell, with the complete state each draws
    /// ([`drawn_state`]), polled from `at`. A display exists only while the
    /// highlight is lit and its cell holds what it draws ([`hold_lines`]).
    Blocks {
        cells: Vec<([i32; 3], String)>,
        at: [i32; 3],
        /// The cells hold a block the objective's activation places (a
        /// collect's chest), not one the world carries.
        staged: bool,
        /// **The lit set** (spec-0117 §3b): the boxes of walkable cells within
        /// range from which the block is in sight, computed at build and
        /// tested by the poll in place of a sphere. `None` for a block whose
        /// set this build does not compute (a `collect`'s container).
        lit: Option<Vec<([i32; 3], [i32; 3])>>,
    },
    /// Every body carrying this tag (a wave), or the NPC's body.
    Bodies { tag: String, npc: bool },
    /// Every runtime marker carrying this tag (a recovery stake's display,
    /// spec-0117 §3b): each lit by its own `Glowing` flag while a player is
    /// within range of it. Its cell is chosen at runtime, so its lit set is a
    /// sphere about it.
    Runtime {
        /// The markers' tag.
        tag: String,
    },
    /// A holder entity a `collect` takes from (spec-0111): an item frame or
    /// an armour stand, lit by its own `Glowing` flag (`Entity.load` reads it
    /// on every entity) in the highlight team's colour. Not a living body, so
    /// no effect reaches it.
    Holder {
        /// The holder's own tag (`OwnedHolder::tag`).
        tag: String,
        /// Its cell.
        at: [i32; 3],
        /// Its own body function (`emit::holders::holder_fn`).
        body_fn: String,
    },
}

/// One declared highlight.
#[derive(Clone, Debug)]
pub(super) struct HlAct {
    /// `o_<safe>` for an objective, `t_<safe>` for a trigger.
    pub safe: String,
    /// The objective's id, for an objective.
    pub objective: Option<String>,
    /// The trigger's id, for a trigger.
    pub trigger: Option<String>,
    /// The range in blocks.
    pub within: u8,
    pub body: HlBody,
    /// The armed condition, as `execute` sub-commands ending in a space.
    pub armed: String,
}

/// The campaign's guidance, derived once per build.
#[derive(Clone, Debug, Default)]
pub(super) struct Guide {
    /// Every watched objective, in document order.
    pub watched: Vec<Watched>,
    /// Every declared highlight that lights something.
    pub highlights: Vec<HlAct>,
    /// Whether the campaign carries a compass (any objective is tracked).
    pub compass: bool,
    /// Whether the campaign carries a journal (any objective is announced, or a
    /// manual is declared).
    pub journal: bool,
}

impl Guide {
    /// The tracked objectives, in ordinal order.
    pub fn tracked(&self) -> impl Iterator<Item = &Watched> {
        self.watched.iter().filter(|w| w.ordinal.is_some())
    }

    /// The announced objectives, in document order.
    pub fn announced(&self) -> impl Iterator<Item = &Watched> {
        self.watched.iter().filter(|w| w.announced)
    }

    /// Whether any guidance machinery is emitted at all.
    pub fn any(&self) -> bool {
        self.compass || self.journal || !self.highlights.is_empty()
    }

    /// The highlight declared on objective `oid`, if any.
    pub fn highlight_of(&self, oid: &str) -> Option<&HlAct> {
        self.highlights
            .iter()
            .find(|h| h.objective.as_deref() == Some(oid))
    }

    /// The state holder of a watched objective: `0` closed, `1` pending, `2`
    /// complete.
    pub fn state_holder(safe: &str) -> String {
        format!("#gs_{safe}")
    }
}

/// The blocks whose block model draws nothing, or only part of the thing: a
/// highlight draws their item instead (`crates/delvec/data/block-entity-renderers-1.21.11.json`,
/// `tools/maintenance/extract-block-entity-renderers.py`).
fn drawn_by_renderer(block: &str) -> bool {
    static SET: std::sync::OnceLock<BTreeSet<String>> = std::sync::OnceLock::new();
    SET.get_or_init(|| {
        let v: Value = serde_json::from_str(include_str!(
            "../../../data/block-entity-renderers-1.21.11.json"
        ))
        .expect("the block-entity renderer table parses");
        let mut s: BTreeSet<String> = v["derived"]
            .as_array()
            .expect("derived")
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect();
        s.extend(v["authored"].as_object().expect("authored").keys().cloned());
        s
    })
    .contains(block)
}

/// `minecraft:<id>` for a block id written with or without its namespace.
fn namespaced(id: &str) -> String {
    if id.contains(':') {
        id.to_string()
    } else {
        format!("minecraft:{id}")
    }
}

/// Derive the campaign's guidance. `blocks` is the assembled world, read for
/// the state of a block a highlight lights that the campaign did not place;
/// `holders` the holder entities the pieces carry, read for a `collect` taken
/// from one.
pub(super) fn guide(
    plan: &Plan,
    blocks: &crate::compiler::blockstate::BlockMap,
    holders: &[crate::compiler::holders::OwnedHolder],
    world: &crate::compiler::nav::World,
) -> Guide {
    let c = plan.campaign;
    let guidance = &c.quests.content.guidance;
    let mut g = Guide::default();
    let mut ordinal = 0usize;
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        let qa = quest_active_score(q.id.as_str());
        for (oi, o) in q.objectives.iter().enumerate() {
            let announced = o.announced(guidance);
            let tracked = o.tracked(guidance);
            if !announced && o.highlight().is_none() {
                continue;
            }
            let ord = if tracked {
                ordinal += 1;
                Some(ordinal)
            } else {
                None
            };
            let target = if tracked {
                match o {
                    Objective::TalkTo { npc, .. } => Some(Target::Body(format!(
                        "dw_npc_{}",
                        plan::safe_local(npc.as_str())
                    ))),
                    Objective::Kill { wave, .. } => Some(Target::Wave {
                        tag: plan::wave_tag(wave.as_str()),
                        anchor: crate::compiler::promise::objective_place(plan, q, o),
                    }),
                    _ => crate::compiler::promise::objective_place(plan, q, o).map(Target::Cell),
                }
            } else {
                None
            };
            g.watched.push(Watched {
                quest: qi,
                objective: oi,
                id: o.id().as_str().to_string(),
                safe: plan::safe_local(o.id().as_str()),
                guard: told_guard(plan, o, &qa),
                announced,
                ordinal: ord,
                target,
            });
        }
    }
    g.compass = ordinal > 0;
    g.journal = g.watched.iter().any(|w| w.announced) || guidance.manual.is_some();
    g.highlights = highlight_acts(plan, blocks, holders, &g, world);
    g
}

fn highlight_acts(
    plan: &Plan,
    blocks: &crate::compiler::blockstate::BlockMap,
    holders: &[crate::compiler::holders::OwnedHolder],
    g: &Guide,
    world: &crate::compiler::nav::World,
) -> Vec<HlAct> {
    use crate::compiler::pressable::Owner;
    use delvewright_dsl::TriggerOn;
    let c = plan.campaign;
    let block_at = |cell: [i32; 3]| -> Option<String> {
        blocks
            .get(&cell)
            .map(|b| b.as_str().to_string())
            .filter(|b| !delvewright_dsl::blockshape::is_air(b))
    };
    // A pressable's block: its cells and the state each draws, and the lit set
    // its highlight is tested against (spec-0117 §3b).
    let press_body = |p: &crate::compiler::pressable::PressPlan, within: u8| -> HlBody {
        let mut cells = vec![(p.cell, p.block.clone())];
        if crate::compiler::pressable::is_door(&p.block) {
            cells.push((
                [p.cell[0], p.cell[1] + 1, p.cell[2]],
                crate::compiler::pressable::upper_half(&p.block),
            ));
        }
        HlBody::Blocks {
            cells,
            at: p.cell,
            staged: false,
            lit: Some(lit_set(plan, world, p, within)),
        }
    };
    let mut out = Vec::new();
    for w in &g.watched {
        let q = &c.quests.content.quests[w.quest];
        let o = &q.objectives[w.objective];
        let Some(h) = o.highlight() else { continue };
        let area = plan.quest_area(q.id.as_str());
        let at = |anchor: &str| {
            area.and_then(|a| plan.point(a, anchor))
                .or_else(|| plan.point_any(anchor))
        };
        let body = match o {
            Objective::Interact { id, .. } => {
                let Some(p) = crate::compiler::pressable::press_of_objective(plan, id.as_str())
                else {
                    continue;
                };
                press_body(p, h.within)
            }
            Objective::Collect {
                id,
                anchor,
                dropped_by: None,
                ..
            } => {
                let adopted = plan
                    .collect_fills
                    .iter()
                    .find(|f| f.objective_id == id.as_str())
                    .map(|f| f.cell);
                // A collect taken from a holder entity lights the holder
                // itself: its cell holds air in the block map.
                let holder = holders
                    .iter()
                    .find(|h| h.collect.as_deref() == Some(id.as_str()));
                if let Some(h) = holder {
                    HlBody::Holder {
                        tag: h.tag(),
                        at: h.holder.cell,
                        body_fn: super::holders::holder_fn(h),
                    }
                } else {
                    let (cell, state, staged) = match adopted {
                        Some(cell) => match block_at(cell) {
                            Some(s) => (cell, s, false),
                            None => continue,
                        },
                        None => match at(anchor.as_str()) {
                            Some(cell) => (cell, "minecraft:chest".to_string(), true),
                            None => continue,
                        },
                    };
                    HlBody::Blocks {
                        cells: vec![(cell, state)],
                        at: cell,
                        staged,
                        lit: None,
                    }
                }
            }
            Objective::TalkTo { npc, .. } => HlBody::Bodies {
                tag: format!("dw_npc_{}", plan::safe_local(npc.as_str())),
                npc: true,
            },
            Objective::Kill { wave, .. } => HlBody::Bodies {
                tag: plan::wave_tag(wave.as_str()),
                npc: false,
            },
            _ => continue,
        };
        out.push(HlAct {
            safe: format!("o_{}", w.safe),
            objective: Some(w.id.clone()),
            trigger: None,
            within: h.within,
            body,
            armed: format!(
                "if score {} {GUIDE} matches 1 ",
                Guide::state_holder(&w.safe)
            ),
        });
    }
    for t in &c.quests.content.triggers {
        let Some(h) = t.highlight else { continue };
        let (once, forbid, flags) = trigger_poll_guards(plan, t);
        let armed = format!("{once}{forbid}{flags}");
        let body = match &t.on {
            TriggerOn::Use | TriggerOn::Strike => {
                match crate::compiler::pressable::press_of_trigger(plan, t.id.as_str()) {
                    Some(p) => press_body(p, h.within),
                    // A strike at an NPC's stand lights the NPC it rides.
                    None => match npc_ridden_by(plan, t) {
                        Some(n) => HlBody::Bodies {
                            tag: n.tag.clone(),
                            npc: true,
                        },
                        None => continue,
                    },
                }
            }
            TriggerOn::StrikeNpc { npc } => HlBody::Bodies {
                tag: format!("dw_npc_{}", plan::safe_local(npc.as_str())),
                npc: true,
            },
            // `DW1016` refuses the rest at validation.
            _ => continue,
        };
        out.push(HlAct {
            safe: format!("t_{}", plan::safe_local(t.id.as_str())),
            objective: None,
            trigger: Some(t.id.as_str().to_string()),
            within: h.within,
            body,
            armed,
        });
    }
    // spec-0117 §3b: every other pressable — a shortcut's unlock, a bonfire, a
    // shop, each disarm — lit while open, from where its block is in sight.
    for p in plan.presses() {
        let Some(h) = p.highlight else { continue };
        let armed = match &p.owner {
            Owner::Interact { .. } | Owner::Trigger { .. } | Owner::GateAnswer { .. } => continue,
            other => super::press::owner_guards(plan, other).0,
        };
        out.push(HlAct {
            safe: format!("p_{}", p.owner.safe()),
            objective: None,
            trigger: None,
            within: h.within,
            body: press_body(p, h.within),
            armed,
        });
    }
    // A recovery stake's marker: lit near, never from afar.
    if let Some(within) = c
        .quests
        .content
        .stakes
        .iter()
        .filter(|s| s.max_live() > 0)
        .filter_map(|s| s.highlight.map(|h| h.within))
        .max()
    {
        out.push(HlAct {
            safe: "stake".to_string(),
            objective: None,
            trigger: None,
            within,
            body: HlBody::Runtime {
                tag: super::stake::STK_DISPLAY_TAG.to_string(),
            },
            armed: String::new(),
        });
    }
    // Every display draws the complete state, and the same string is what its
    // cell is held to, so the copy and the check cannot disagree.
    for h in &mut out {
        if let HlBody::Blocks { cells, .. } = &mut h.body {
            for (_, state) in cells.iter_mut() {
                *state = drawn_state(state);
            }
        }
    }
    out
}

/// **The lit set of a pressable's highlight** (spec-0117 §3b): every walkable
/// cell within `within` blocks of the block from which the block is in sight
/// (the one sight rule, [`crate::compiler::framing::in_sight`], over the nav
/// world's camera-blocking cells, from a standing body's eye), and — for a
/// shortcut's unlock or a shortcut door's own answer — on the side the press is
/// made from. Returned as boxes of cells, each one row along x merged across z,
/// in ascending order (ADR-0006).
pub(super) fn lit_set(
    plan: &Plan,
    world: &crate::compiler::nav::World,
    p: &crate::compiler::pressable::PressPlan,
    within: u8,
) -> Vec<([i32; 3], [i32; 3])> {
    use crate::compiler::pressable::Owner;
    let r = i32::from(within);
    let target =
        crate::compiler::framing::CellBox::spanning(p.cell, *p.cells().last().unwrap_or(&p.cell));
    // The side the press is made from, where the press has one.
    let side: Option<(usize, i32, i32)> = match &p.owner {
        Owner::Shortcut { id } => plan
            .shortcuts
            .iter()
            .find(|s| &s.id == id)
            .and_then(|s| s.sealed_side.as_ref())
            .map(|sd| {
                // The far side: opposite the sealed one.
                let (lo, hi) = (sd.gate_lo[sd.axis], sd.gate_hi[sd.axis]);
                if sd.dir < 0 {
                    (sd.axis, 1, hi)
                } else {
                    (sd.axis, -1, lo)
                }
            }),
        Owner::GateAnswer { gate, .. } => plan
            .shortcuts
            .iter()
            .find(|s| &s.gate_anchor == gate)
            .and_then(|s| s.sealed_side.as_ref())
            .map(|sd| {
                let (lo, hi) = (sd.gate_lo[sd.axis], sd.gate_hi[sd.axis]);
                if sd.dir < 0 {
                    (sd.axis, -1, lo)
                } else {
                    (sd.axis, 1, hi)
                }
            }),
        _ => None,
    };
    let on_side = |c: [i32; 3]| match side {
        Some((axis, 1, bound)) => c[axis] > bound,
        Some((axis, _, bound)) => c[axis] < bound,
        None => true,
    };
    let centre = [
        f64::from(p.cell[0]) + 0.5,
        f64::from(p.cell[1]) + 0.5,
        f64::from(p.cell[2]) + 0.5,
    ];
    let mut cells: Vec<[i32; 3]> = Vec::new();
    for x in p.cell[0] - r..=p.cell[0] + r {
        for y in p.cell[1] - r..=p.cell[1] + r {
            for z in p.cell[2] - r..=p.cell[2] + r {
                let c = [x, y, z];
                let d = ((f64::from(x) + 0.5 - centre[0]).powi(2)
                    + (f64::from(y) + 0.5 - centre[1]).powi(2)
                    + (f64::from(z) + 0.5 - centre[2]).powi(2))
                .sqrt();
                if d > f64::from(within) || !on_side(c) || !world.is_standable(c) {
                    continue;
                }
                let eye = [
                    f64::from(x) + 0.5,
                    world.feet_y(c) + delvewright_dsl::metrics::PLAYER_EYE_HEIGHT,
                    f64::from(z) + 0.5,
                ];
                // The vantage is no occluder (the rule `DW0863` reads): the
                // cell the body stands in and its eye's own cell never stop a
                // line; the block's own cells are the target's.
                let eye_cell = [
                    eye[0].floor() as i32,
                    eye[1].floor() as i32,
                    eye[2].floor() as i32,
                ];
                if crate::compiler::framing::in_sight(eye, &target, &|q| {
                    q != c && q != eye_cell && world.blocks_camera(q)
                })
                .seen()
                {
                    cells.push(c);
                }
            }
        }
    }
    cells.sort();
    boxes_of(&cells)
}

/// Cells as boxes: each `(y, z)` row's runs along x, then runs equal in x and
/// y merged across consecutive z. Deterministic: ascending.
fn boxes_of(cells: &[[i32; 3]]) -> Vec<([i32; 3], [i32; 3])> {
    use std::collections::BTreeMap;
    let mut rows: BTreeMap<(i32, i32), Vec<i32>> = BTreeMap::new();
    for c in cells {
        rows.entry((c[1], c[2])).or_default().push(c[0]);
    }
    // (y, x0, x1) -> list of z, in order
    let mut runs: BTreeMap<(i32, i32, i32), Vec<i32>> = BTreeMap::new();
    for ((y, z), mut xs) in rows {
        xs.sort();
        let mut i = 0;
        while i < xs.len() {
            let mut j = i;
            while j + 1 < xs.len() && xs[j + 1] == xs[j] + 1 {
                j += 1;
            }
            runs.entry((y, xs[i], xs[j])).or_default().push(z);
            i = j + 1;
        }
    }
    let mut out = Vec::new();
    for ((y, x0, x1), mut zs) in runs {
        zs.sort();
        let mut i = 0;
        while i < zs.len() {
            let mut j = i;
            while j + 1 < zs.len() && zs[j + 1] == zs[j] + 1 {
                j += 1;
            }
            out.push(([x0, y, zs[i]], [x1, y, zs[j]]));
            i = j + 1;
        }
    }
    out.sort();
    out
}

/// The complete state a block display of `state` draws: every property the
/// state leaves out filled with the pinned default the game resolves it to
/// (`BlockRegistry::default_state`), written `minecraft:id[k=v,…]` in
/// property order. A `block_display`'s `block_state` fills an unwritten
/// property the same way, so this is the state the display shows.
pub(super) fn drawn_state(state: &str) -> String {
    let (name, written) = match state.split_once('[') {
        Some((n, rest)) => (n, rest.trim_end_matches(']')),
        None => (state, ""),
    };
    let name = namespaced(name);
    let mut props: BTreeMap<String, String> = written
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    if let Some(d) = crate::schem::blocks::BlockRegistry::v1_21_11().default_state(&name) {
        for (k, v) in d {
            props.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
    if props.is_empty() {
        return name;
    }
    let kv: Vec<String> = props.iter().map(|(k, v)| format!("{k}={v}")).collect();
    format!("{name}[{}]", kv.join(","))
}

/// The block predicate a display of `state` holds its cell to: what the
/// display draws, and nothing it does not. A `block_display` draws the whole
/// state; an `item_display` draws the block's item turned by `facing`, so its
/// cell is held to the block and its `facing`.
pub(super) fn drawn_predicate(state: &str) -> String {
    if display_entity(state) == "minecraft:block_display" {
        return state.to_string();
    }
    let (name, props) = match state.split_once('[') {
        Some((n, rest)) => (n, rest.trim_end_matches(']')),
        None => (state, ""),
    };
    match props
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == "facing")
    {
        Some((_, v)) => format!("{}[facing={v}]", namespaced(name)),
        None => namespaced(name),
    }
}

/// The selector of a highlighted body: every entity carrying the tag that can
/// glow (an NPC's interaction hitbox carries the tag too, and draws nothing).
fn body_selector(tag: &str, npc: bool) -> String {
    if npc {
        format!("@e[tag={tag},type=!minecraft:interaction]")
    } else {
        format!("@e[tag={tag}]")
    }
}

/// The lines that light (or darken) a holder entity: its `Glowing` flag
/// (`Entity.load` reads `Glowing` on every entity; the client draws the
/// outline of any entity whose flag is set, `EntityRenderer.extractRenderState`
/// writing its team colour to `outlineColor`, which `ItemFrameRenderer.submit`
/// passes to the frame and its item and a living body's renderer to its model
/// — pinned 1.21.11 client jar) and the highlight team for the colour.
fn holder_glow(tag: &str, on: bool) -> Vec<String> {
    let sel = format!("@e[tag={tag}]");
    if on {
        vec![
            format!("execute as {sel} run data merge entity @s {{Glowing:1b}}"),
            format!("team join {HL_TEAM} {sel}"),
        ]
    } else {
        vec![
            format!("execute as {sel} run data merge entity @s {{Glowing:0b}}"),
            format!("team leave {sel}"),
        ]
    }
}

/// The SNBT `block_state` compound of a block written `minecraft:id[k=v,…]`.
fn block_state_snbt(state: &str) -> String {
    let (name, props) = match state.split_once('[') {
        Some((n, rest)) => (n, rest.trim_end_matches(']')),
        None => (state, ""),
    };
    let mut out = format!("{{Name:{}", snbt_string(&namespaced(name)));
    let pairs: Vec<String> = props
        .split(',')
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| format!("{k}:{}", snbt_string(v)))
        .collect();
    if !pairs.is_empty() {
        out.push_str(&format!(",Properties:{{{}}}", pairs.join(",")));
    }
    out.push('}');
    out
}

/// The item a block drawn by a renderer shows as: its wall variant's item is
/// the standing one's.
fn renderer_item(block: &str) -> String {
    namespaced(&block.replace("_wall_", "_"))
}

/// The left rotation that turns an item model to face like the block's
/// `facing` property (a quarter turn about Y per step from south).
fn facing_rotation(state: &str) -> &'static str {
    let facing = state
        .split_once('[')
        .map(|(_, p)| p.trim_end_matches(']'))
        .and_then(|p| {
            p.split(',')
                .filter_map(|kv| kv.split_once('='))
                .find(|(k, _)| *k == "facing")
                .map(|(_, v)| v.to_string())
        });
    match facing.as_deref() {
        Some("west") => "[0f,0.70710677f,0f,0.70710677f]",
        Some("north") => "[0f,1f,0f,0f]",
        Some("east") => "[0f,-0.70710677f,0f,0.70710677f]",
        _ => "[0f,0f,0f,1f]",
    }
}

/// The display entity a highlight draws `state` with (§7.2): a
/// `minecraft:block_display` of the block, or a `minecraft:item_display` of its
/// item for a block whose model draws nothing or only part of the thing.
pub(super) fn display_entity(state: &str) -> &'static str {
    let base = state.split('[').next().unwrap_or(state);
    if drawn_by_renderer(&namespaced(base)) {
        "minecraft:item_display"
    } else {
        "minecraft:block_display"
    }
}

/// Where a highlight display of `state` stands to cover `cell`, as the three
/// coordinates of its summon. A `block_display` draws its block from its
/// origin as the minimum corner, so it stands at the cell's corner, written
/// `x.0` so the summon cannot centre it; an `item_display` draws its item
/// centred on its origin, so it stands at the cell's centre on all three axes.
pub(super) fn display_origin(cell: [i32; 3], state: &str) -> String {
    if display_entity(state) == "minecraft:item_display" {
        format!(
            "{} {} {}",
            center(cell[0]),
            center(cell[1]),
            center(cell[2])
        )
    } else {
        format!(
            "{} {} {}",
            corner(cell[0]),
            corner(cell[1]),
            corner(cell[2])
        )
    }
}

/// The summon of highlight `safe`'s display of its `i`th cell, `cell`,
/// drawing `state` (§7.2), lit from the moment it exists: a display is
/// summoned only while its highlight is lit ([`hold_lines`]). It carries
/// [`cell_tag`] beside the highlight's own tag.
pub(super) fn display_summon(safe: &str, i: usize, cell: [i32; 3], state: &str) -> String {
    let base = state.split('[').next().unwrap_or(state);
    let tags = format!(
        "Glowing:1b,Tags:[{FIXTURE_NBT}\"dw_hl\",\"dw_hl_{safe}\",\"{}\"]",
        cell_tag(safe, i)
    );
    let at = display_origin(cell, state);
    if display_entity(state) == "minecraft:item_display" {
        format!(
            "summon minecraft:item_display {at} {{{tags},item:{{id:{},count:1}},item_display:\"none\",transformation:{{left_rotation:{},right_rotation:[0f,0f,0f,1f],scale:[1.002f,1.002f,1.002f],translation:[0f,0f,0f]}},glow_color_override:{HL_RGB}}}",
            snbt_string(&renderer_item(base)),
            facing_rotation(state)
        )
    } else {
        format!(
            "summon minecraft:block_display {at} {{{tags},block_state:{},transformation:{{left_rotation:[0f,0f,0f,1f],right_rotation:[0f,0f,0f,1f],scale:[1.002f,1.002f,1.002f],translation:[-0.001f,-0.001f,-0.001f]}},glow_color_override:{HL_RGB}}}",
            block_state_snbt(state)
        )
    }
}

/// The tag of highlight `safe`'s display of its `i`th cell: `dw_hl_<a>.<i>`.
/// The `.` is never in a `safe_local` name, so no other highlight's tag can
/// spell it.
pub(super) fn cell_tag(safe: &str, i: usize) -> String {
    format!("dw_hl_{safe}.{i}")
}

/// The body of `hl_hold_<a>`, the one place a block highlight's displays are
/// summoned: run while the highlight is lit, it keeps exactly one display on
/// each cell that holds the state the display draws, and none on a cell that
/// does not. A display is never left standing over a cell that changed — a
/// lever pulled, a button pressed, a block taken — and none exists while the
/// highlight is dark: `hl_off_<a>` kills them all, and the poll turns the
/// highlight off on the tick its act stops being armed. Empty for a body
/// highlight.
pub(super) fn hold_lines(h: &HlAct) -> Vec<String> {
    let HlBody::Blocks { cells, .. } = &h.body else {
        return Vec::new();
    };
    let mut v = Vec::new();
    for (i, (cell, state)) in cells.iter().enumerate() {
        let pred = drawn_predicate(state);
        let [x, y, z] = *cell;
        let sel = format!("@e[tag={}]", cell_tag(&h.safe, i));
        v.push(format!(
            "execute unless block {x} {y} {z} {pred} run kill {sel}"
        ));
        v.push(format!(
            "execute if block {x} {y} {z} {pred} unless entity {sel} run {}",
            display_summon(&h.safe, i, *cell, state)
        ));
    }
    v
}

/// The lines an objective's completion runs for its highlight: the same
/// `hl_off_<a>` the poll turns it dark with, so the displays are removed and
/// the bodies' glow cleared through the one way a highlight goes out.
pub(super) fn completion_lines(g: &Guide, ns: &str, oid: &str) -> Vec<String> {
    match g.highlight_of(oid) {
        Some(h) => vec![format!("function {ns}:hl_off_{}", h.safe)],
        None => Vec::new(),
    }
}

/// The `setup` lines: the scoreboards and the highlight team.
pub(super) fn setup_lines(g: &Guide) -> Vec<String> {
    let mut v = Vec::new();
    if !g.any() {
        return v;
    }
    v.push(format!("scoreboard objectives add {GUIDE} dummy"));
    v.push(format!("scoreboard players set {EPOCH} {GUIDE} 0"));
    if g.compass || g.journal {
        for (o, kind) in [(SEEN, "dummy"), (DEATHS, "deathCount")] {
            v.push(format!("scoreboard objectives add {o} {kind}"));
        }
    }
    if g.compass {
        for o in [TRACK, TOLD, USE_LATCH, NEXT, ON_BODY] {
            v.push(format!("scoreboard objectives add {o} dummy"));
        }
        v.push(format!("scoreboard objectives add {PICK} trigger"));
    }
    if !g.highlights.is_empty() {
        v.push(format!("team add {HL_TEAM}"));
        v.push(format!("team modify {HL_TEAM} color {HL_COLOR}"));
    }
    v
}

/// The `tick` lines (§6.3, §6.4, §7.3, §8.3, §8.4).
pub(super) fn tick_lines(g: &Guide, ns: &str) -> Vec<String> {
    let mut v = Vec::new();
    if !g.any() {
        return v;
    }
    if g.compass || g.journal {
        // Each player's guidance entries, once, on their first tick.
        v.push(format!(
            "execute as @a[tag=!{SEEDED_TAG}] run function {ns}:guide_seed"
        ));
    }
    v.push(format!("function {ns}:guide_scan"));
    for h in &g.highlights {
        v.push(format!("function {ns}:hl_poll_{}", h.safe));
    }
    if g.compass || g.journal {
        // A dropped engine item is removed, and the epoch moves so the next
        // refresh hands its owner a fresh one.
        v.push(format!("function {ns}:guide_strays"));
        // A respawn: a living player with a death since the last refresh.
        v.push(format!(
            "execute as @e[type=minecraft:player,scores={{{DEATHS}=1..}}] run function {ns}:guide_respawn"
        ));
        v.push(format!(
            "execute as @a[scores={{dw.classed=1}}] unless score @s {SEEN} = {EPOCH} {GUIDE} run function {ns}:guide_refresh"
        ));
    }
    if g.compass {
        // A pick is read on the tick (the trigger is enabled with the other
        // answer channels, `enable_lines`).
        v.push(format!(
            "execute as @a[scores={{{PICK}=1..}}] run function {ns}:compass_pick"
        ));
        v.push(format!(
            "scoreboard players remove @a[scores={{{USE_LATCH}=1..}}] {USE_LATCH} 1"
        ));
        if g.tracked()
            .any(|w| matches!(w.target, Some(Target::Body(_)) | Some(Target::Wave { .. })))
        {
            v.push(format!(
                "execute if score {CLOCK} {GUIDE} matches 0 as @a[scores={{{ON_BODY}=1}}] run function {ns}:compass_aim_followed"
            ));
        }
    }
    v
}

/// The `tick` lines that enable the menu's answer channel: the buttons run
/// `/trigger`, which a player can use only while it is enabled for them.
/// Emitted with the other answer channels, ahead of the respawn wait's lock,
/// so a waiting player's channel stays locked.
pub(super) fn enable_lines(g: &Guide) -> Vec<String> {
    if !g.compass {
        return Vec::new();
    }
    vec![format!(
        "scoreboard players enable @a[scores={{dw.classed=1}}] {PICK}"
    )]
}

/// The advancement that sees the engine's compass used (§6.4):
/// `minecraft:using_item` fires every tick it is held in use, and its reward
/// re-arms it.
pub(super) fn use_advancement(ns: &str) -> Value {
    json!({
        "criteria": {
            "use": {
                "trigger": "minecraft:using_item",
                "conditions": {
                    "item": {
                        "items": "minecraft:compass",
                        "predicates": { "minecraft:custom_data": COMPASS_DATA }
                    }
                }
            }
        },
        "rewards": { "function": format!("{ns}:compass_used") }
    })
}

/// The lines `class_apply_<c>` gives with the kit: the compass and the
/// journal, then a refresh at once.
pub(super) fn kit_lines(g: &Guide, ns: &str) -> Vec<String> {
    let mut v = Vec::new();
    if g.compass {
        v.push(format!("function {ns}:compass_give"));
    }
    if g.journal {
        v.push(format!("function {ns}:journal_give"));
    }
    if g.compass || g.journal {
        v.push(format!("function {ns}:guide_refresh"));
    }
    v
}

/// The compass stack, aimed by the `lodestone_tracker` given (the whole
/// component, `{…}`).
fn compass_stack(chrome: &delvewright_dsl::Chrome, tracker: &str) -> String {
    let name = snbt_of(&tr_with(
        &chrome.get(delvewright_dsl::chrome::COMPASS_NAME),
        &[("italic", json!(false))],
    ));
    format!(
        "minecraft:compass[custom_name={name},custom_data={COMPASS_DATA},consumable={{consume_seconds:1000000f,animation:\"none\",has_consume_particles:false}},lodestone_tracker={tracker}]"
    )
}

/// The journal stack as given: empty pages, which the first rewrite fills.
fn journal_stack(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> String {
    let c = plan.campaign;
    let title = c
        .quests
        .content
        .guidance
        .manual
        .as_ref()
        .and_then(|m| m.title.clone())
        .unwrap_or_else(|| chrome.get(delvewright_dsl::chrome::JOURNAL_TITLE));
    let author = delvewright_dsl::l10n::plain(&c.world.content.title).into_owned();
    format!(
        "minecraft:written_book[custom_data={JOURNAL_DATA},item_name={},written_book_content={{title:\"\",author:{},pages:[]}}]",
        snbt_of(&tr(&title)),
        snbt_string(&author)
    )
}

/// Every slot a player carries an item in (`emit::checkpoint::CARRIED_SLOTS`).
fn slots() -> &'static [&'static str] {
    CARRIED_SLOTS
}

/// `execute unless items …` over every carried slot, ending in ` run `.
fn unless_held(item: &str) -> String {
    format!(
        "execute unless items entity @s container.* {item} unless items entity @s armor.* {item} unless items entity @s weapon.offhand {item} run "
    )
}

/// The guidance functions.
pub(super) fn functions(
    g: &Guide,
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut fns: Vec<(String, String)> = Vec::new();
    if !g.any() {
        return fns;
    }

    // --- guide_scan: the state of every watched objective, and the epoch ---
    let mut scan = Vec::new();
    for w in &g.watched {
        let s = Guide::state_holder(&w.safe);
        let l = format!("#gsl_{}", w.safe);
        scan.push(format!(
            "execute store success score {s} {GUIDE}{}",
            w.guard
        ));
        scan.push(format!(
            "execute if score {} {} matches 1 run scoreboard players set {s} {GUIDE} 2",
            plan::PARTY,
            obj_score(&w.id)
        ));
        scan.push(format!(
            "execute unless score {s} {GUIDE} = {l} {GUIDE} run scoreboard players add {EPOCH} {GUIDE} 1"
        ));
        scan.push(format!(
            "scoreboard players operation {l} {GUIDE} = {s} {GUIDE}"
        ));
    }
    if g.compass {
        scan.push(format!("scoreboard players add {CLOCK} {GUIDE} 1"));
        scan.push(format!(
            "execute if score {CLOCK} {GUIDE} matches 20.. run scoreboard players set {CLOCK} {GUIDE} 0"
        ));
    }
    fns.push(("guide_scan".to_string(), lines(&scan)));

    // --- the highlight polls (§7.3) ---
    for h in &g.highlights {
        let hin = format!("#hlin_{}", h.safe);
        let hl = format!("#hl_{}", h.safe);
        let w = h.within;
        let near = format!("@a[distance=..{w},tag=!{CUTSCENE_TAG}]");
        let mut poll = vec![format!("scoreboard players set {hin} {GUIDE} 0")];
        match &h.body {
            // spec-0117 §3b: a pressable's block is lit from its lit set — the
            // walkable cells in range that see it — tested box by box.
            HlBody::Blocks { lit: Some(boxes), .. } => {
                for (lo, hi) in boxes {
                    poll.push(format!(
                        "execute {}if entity @a[x={},y={},z={},dx={},dy={},dz={},tag=!{CUTSCENE_TAG}] run scoreboard players set {hin} {GUIDE} 1",
                        h.armed,
                        lo[0],
                        lo[1],
                        lo[2],
                        hi[0] - lo[0],
                        hi[1] - lo[1],
                        hi[2] - lo[2]
                    ));
                }
            }
            HlBody::Blocks { at, .. } => poll.push(format!(
                "execute {}positioned {} {} {} if entity {near} run scoreboard players set {hin} {GUIDE} 1",
                h.armed,
                center(at[0]),
                center(at[1]),
                center(at[2])
            )),
            // Each marker lights itself, whoever stands near it: its cell is a
            // runtime fact.
            HlBody::Runtime { .. } => {}
            HlBody::Bodies { tag, npc } => poll.push(format!(
                "execute {}as {} at @s if entity {near} run scoreboard players set {hin} {GUIDE} 1",
                h.armed,
                body_selector(tag, *npc)
            )),
            HlBody::Holder { tag, .. } => poll.push(format!(
                "execute {}as @e[tag={tag}] at @s if entity {near} run scoreboard players set {hin} {GUIDE} 1",
                h.armed
            )),
        }
        poll.push(format!(
            "execute if score {hin} {GUIDE} matches 1 unless score {hl} {GUIDE} matches 1 run function {ns}:hl_on_{}",
            h.safe
        ));
        poll.push(format!(
            "execute if score {hin} {GUIDE} matches 0 if score {hl} {GUIDE} matches 1 run function {ns}:hl_off_{}",
            h.safe
        ));
        if let HlBody::Bodies { tag, npc } = &h.body {
            // A body that arrives while the highlight is lit glows from the
            // moment it exists.
            let mut sel = body_selector(tag, *npc);
            sel.insert_str(sel.len() - 1, &format!(",team=!{HL_TEAM}"));
            poll.push(format!(
                "execute if score {hl} {GUIDE} matches 1 if entity {sel} run function {ns}:hl_on_{}",
                h.safe
            ));
        }
        if matches!(h.body, HlBody::Blocks { .. }) {
            // While lit, every cell is held to what its display draws.
            poll.push(format!(
                "execute if score {hl} {GUIDE} matches 1 run function {ns}:hl_hold_{}",
                h.safe
            ));
        }
        if let HlBody::Runtime { tag } = &h.body {
            // A runtime marker is lit and darkened by its own neighbourhood,
            // every tick, one marker at a time.
            poll = vec![
                format!(
                    "execute as @e[tag={tag}] at @s if entity {near} run data merge entity @s {{Glowing:1b,glow_color_override:{HL_RGB}}}"
                ),
                format!(
                    "execute as @e[tag={tag}] at @s unless entity {near} run data merge entity @s {{Glowing:0b}}"
                ),
            ];
        }
        fns.push((format!("hl_poll_{}", h.safe), lines(&poll)));
        let (on, off) = match &h.body {
            HlBody::Runtime { tag } => (
                Vec::new(),
                vec![format!(
                    "execute as @e[tag={tag}] run data merge entity @s {{Glowing:0b}}"
                )],
            ),
            HlBody::Blocks { .. } => (
                vec![format!("function {ns}:hl_hold_{}", h.safe)],
                vec![format!("kill @e[tag=dw_hl_{}]", h.safe)],
            ),
            HlBody::Bodies { tag, npc } => {
                let sel = body_selector(tag, *npc);
                (
                    vec![
                        format!("effect give {sel} minecraft:glowing infinite 0 true"),
                        format!("team join {HL_TEAM} {sel}"),
                    ],
                    vec![
                        format!("effect clear {sel} minecraft:glowing"),
                        format!("team leave {sel}"),
                    ],
                )
            }
            HlBody::Holder { tag, .. } => (holder_glow(tag, true), holder_glow(tag, false)),
        };
        let mut on = on;
        on.push(format!("scoreboard players set {hl} {GUIDE} 1"));
        let mut off = off;
        off.push(format!("scoreboard players set {hl} {GUIDE} 0"));
        fns.push((format!("hl_on_{}", h.safe), lines(&on)));
        fns.push((format!("hl_off_{}", h.safe), lines(&off)));
        if matches!(h.body, HlBody::Blocks { .. }) {
            fns.push((format!("hl_hold_{}", h.safe), lines(&hold_lines(h))));
        }
    }

    if !(g.compass || g.journal) {
        return fns;
    }

    // --- refresh, respawn, strays ---
    let mut refresh = vec![format!(
        "scoreboard players operation @s {SEEN} = {EPOCH} {GUIDE}"
    )];
    if g.compass {
        refresh.push(format!("function {ns}:compass_refresh"));
    }
    if g.journal {
        refresh.push(format!("function {ns}:journal_rewrite"));
    }
    fns.push(("guide_refresh".to_string(), lines(&refresh)));
    let mut seed = vec![
        format!("scoreboard players add @s {SEEN} 0"),
        format!("scoreboard players add @s {DEATHS} 0"),
    ];
    if g.compass {
        for o in [TRACK, TOLD, NEXT, USE_LATCH, ON_BODY] {
            seed.push(format!("scoreboard players add @s {o} 0"));
        }
    }
    seed.push(format!("tag @s add {SEEDED_TAG}"));
    fns.push(("guide_seed".to_string(), lines(&seed)));
    fns.push((
        "guide_respawn".to_string(),
        lines(&[
            format!("scoreboard players set @s {DEATHS} 0"),
            format!("scoreboard players set @s {SEEN} -1"),
        ]),
    ));
    let mut strays = Vec::new();
    for data in [(g.journal, "dw_journal"), (g.compass, "dw_compass")]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, k)| *k)
    {
        let sel = format!(
            "@e[type=minecraft:item,nbt={{Item:{{components:{{\"minecraft:custom_data\":{{{data}:1b}}}}}}}}]"
        );
        strays.push(format!(
            "execute if entity {sel} run scoreboard players add {EPOCH} {GUIDE} 1"
        ));
        strays.push(format!("kill {sel}"));
    }
    fns.push(("guide_strays".to_string(), lines(&strays)));

    if g.compass {
        fns.extend(compass_functions(g, plan, chrome));
    }
    if g.journal {
        fns.push((
            "journal_give".to_string(),
            lines(&[format!(
                "{}give @s {}",
                unless_held(&journal_item()),
                journal_stack(plan, chrome)
            )]),
        ));
        // The slot holding the book is found once, then written once: a book
        // the player carries twice is written where it is found last.
        let mut rewrite = vec![
            format!("function {ns}:journal_give"),
            format!("data remove storage {ns}:guide journal"),
        ];
        for slot in slots() {
            rewrite.push(format!(
                "execute if items entity @s {slot} {} run data modify storage {ns}:guide journal.slot set value \"{slot}\"",
                journal_item()
            ));
        }
        rewrite.push(format!(
            "execute if data storage {ns}:guide journal.slot run function {ns}:journal_write with storage {ns}:guide journal"
        ));
        fns.push(("journal_rewrite".to_string(), lines(&rewrite)));
        let mut write = vec![format!(
            "$item modify entity @s $(slot) {ns}:journal_prefix"
        )];
        for w in g.announced() {
            match w.ordinal {
                Some(k) => {
                    write.push(format!(
                        "$execute{} if score @s {TRACK} matches {k} run item modify entity @s $(slot) {ns}:journal_open_following_{}",
                        w.guard, w.safe
                    ));
                    write.push(format!(
                        "$execute{} unless score @s {TRACK} matches {k} run item modify entity @s $(slot) {ns}:journal_open_{}",
                        w.guard, w.safe
                    ));
                }
                None => write.push(format!(
                    "$execute{} run item modify entity @s $(slot) {ns}:journal_open_{}",
                    w.guard, w.safe
                )),
            }
        }
        for w in g.announced() {
            write.push(format!(
                "$execute if score {} {} matches 1 run item modify entity @s $(slot) {ns}:journal_done_{}",
                plan::PARTY,
                obj_score(&w.id),
                w.safe
            ));
        }
        fns.push(("journal_write".to_string(), lines(&write)));
    }
    let _ = c;
    fns
}

fn compass_functions(
    g: &Guide,
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut fns = Vec::new();
    let tracked: Vec<&Watched> = g.tracked().collect();
    let spin = compass_stack(chrome, "{tracked:false}");
    fns.push((
        "compass_give".to_string(),
        lines(&[format!("{}give @s {spin}", unless_held(&compass_item()))]),
    ));

    // compass_refresh: give if missing, resolve the followed, tell, aim.
    let mut r = vec![
        format!("function {ns}:compass_give"),
        format!("scoreboard players set @s {NEXT} 0"),
    ];
    for w in &tracked {
        let k = w.ordinal.expect("tracked");
        r.push(format!(
            "execute if score @s {TRACK} matches {k}{} run scoreboard players set @s {NEXT} {k}",
            w.guard
        ));
    }
    for w in &tracked {
        let k = w.ordinal.expect("tracked");
        r.push(format!(
            "execute if score @s {NEXT} matches 0{} run scoreboard players set @s {NEXT} {k}",
            w.guard
        ));
    }
    r.push(format!(
        "scoreboard players operation @s {TRACK} = @s {NEXT}"
    ));
    r.push(format!(
        "execute unless score @s {TOLD} = @s {TRACK} run function {ns}:compass_tell"
    ));
    r.push(format!("function {ns}:compass_aim_followed"));
    fns.push(("compass_refresh".to_string(), lines(&r)));

    // compass_tell: the chat line (§6.5).
    let mut tell = vec![format!(
        "scoreboard players operation @s {TOLD} = @s {TRACK}"
    )];
    tell.push(format!(
        "execute if score @s {TRACK} matches 0 run tellraw @s {}",
        tr_with(
            &chrome.get(delvewright_dsl::chrome::COMPASS_NOTHING),
            &[("color", json!("gray"))]
        )
    ));
    for w in &tracked {
        let k = w.ordinal.expect("tracked");
        let o = &c.quests.content.quests[w.quest].objectives[w.objective];
        let title = o.title().unwrap_or_default();
        tell.push(format!(
            "execute if score @s {TRACK} matches {k} run tellraw @s {}",
            tr_with(
                &chrome.get(delvewright_dsl::chrome::COMPASS_FOLLOWING),
                &[
                    ("color", json!("aqua")),
                    (
                        "with",
                        json!([tr_with(title, &[("color", json!("white"))])])
                    ),
                ]
            )
        ));
    }
    tell.push(format!("scoreboard players set #gopen {GUIDE} 0"));
    for w in &tracked {
        tell.push(format!(
            "execute{} run scoreboard players add #gopen {GUIDE} 1",
            w.guard
        ));
    }
    tell.push(format!(
        "scoreboard players operation #gothers {GUIDE} = #gopen {GUIDE}"
    ));
    tell.push(format!("scoreboard players remove #gothers {GUIDE} 1"));
    tell.push(format!(
        "execute if score #gopen {GUIDE} matches 2.. run tellraw @s {}",
        tr_with(
            &chrome.get(delvewright_dsl::chrome::COMPASS_OTHERS),
            &[
                ("color", json!("gray")),
                (
                    "with",
                    json!([
                        { "score": { "name": "#gothers", "objective": GUIDE } },
                        { "keybind": "key.use" }
                    ])
                ),
            ]
        )
    ));
    fns.push(("compass_tell".to_string(), lines(&tell)));

    // compass_aim_followed: dispatch on the followed ordinal.
    let mut aim = vec![
        format!("scoreboard players set @s {ON_BODY} 0"),
        format!("execute if score @s {TRACK} matches 0 run function {ns}:compass_spin"),
    ];
    for w in &tracked {
        let k = w.ordinal.expect("tracked");
        match &w.target {
            Some(Target::Cell(p)) => aim.push(format!(
                "execute if score @s {TRACK} matches {k} run function {ns}:compass_aim {{x:{},y:{},z:{}}}",
                p[0], p[1], p[2]
            )),
            Some(Target::Body(tag)) => {
                aim.push(format!(
                    "execute if score @s {TRACK} matches {k} run scoreboard players set @s {ON_BODY} 1"
                ));
                aim.push(format!(
                    "execute if score @s {TRACK} matches {k} run function {ns}:compass_aim_body_{}",
                    w.safe
                ));
                let sel = body_selector(tag, true).replace(']', ",limit=1]");
                fns.push((
                    format!("compass_aim_body_{}", w.safe),
                    lines(&[
                        format!(
                            "execute store result storage {ns}:guide aim.x int 1 run data get entity {sel} Pos[0]"
                        ),
                        format!(
                            "execute store result storage {ns}:guide aim.y int 1 run data get entity {sel} Pos[1]"
                        ),
                        format!(
                            "execute store result storage {ns}:guide aim.z int 1 run data get entity {sel} Pos[2]"
                        ),
                        format!(
                            "execute if entity {sel} run function {ns}:compass_aim with storage {ns}:guide aim"
                        ),
                        format!("execute unless entity {sel} run function {ns}:compass_spin"),
                    ]),
                ));
            }
            Some(Target::Wave { tag, anchor }) => {
                aim.push(format!(
                    "execute if score @s {TRACK} matches {k} run scoreboard players set @s {ON_BODY} 1"
                ));
                aim.push(format!(
                    "execute if score @s {TRACK} matches {k} run function {ns}:compass_aim_wave_{}",
                    w.safe
                ));
                fns.push((
                    format!("compass_aim_wave_{}", w.safe),
                    lines(&wave_aim_lines(ns, tag, *anchor)),
                ));
            }
            None => aim.push(format!(
                "execute if score @s {TRACK} matches {k} run function {ns}:compass_spin"
            )),
        }
    }
    fns.push(("compass_aim_followed".to_string(), lines(&aim)));

    // compass_aim (macro) and compass_spin: replace the compass where it lies.
    let aimed = compass_stack(
        chrome,
        "{target:{pos:[I;$(x),$(y),$(z)],dimension:\"minecraft:overworld\"},tracked:false}",
    );
    let mut am = Vec::new();
    let mut sp = Vec::new();
    for slot in slots() {
        am.push(format!(
            "$execute if items entity @s {slot} {} run item replace entity @s {slot} with {aimed}",
            compass_item()
        ));
        sp.push(format!(
            "execute if items entity @s {slot} {} run item replace entity @s {slot} with {spin}",
            compass_item()
        ));
    }
    fns.push(("compass_aim".to_string(), lines(&am)));
    fns.push(("compass_spin".to_string(), lines(&sp)));

    // compass_used: one press opens the menu once (§6.4).
    fns.push((
        "compass_used".to_string(),
        lines(&[
            format!("advancement revoke @s only {ns}:{COMPASS_USE_ADVANCEMENT}"),
            // A watcher is out of play everywhere (spec-0077 §5).
            format!(
                "execute unless entity @s[tag={CUTSCENE_TAG}] unless score @s {USE_LATCH} matches 1.. run function {ns}:compass_menu"
            ),
            format!("scoreboard players set @s {USE_LATCH} 2"),
        ]),
    ));

    // compass_menu: a dialog built in storage, one button per pending tracked
    // objective; shown when it has one, `compass.nothing` otherwise.
    let menu_title = snbt_of(&tr(&chrome.get(delvewright_dsl::chrome::COMPASS_CHOOSE)));
    let mut menu = vec![format!(
        "data modify storage {ns}:guide menu.dialog set value {{type:\"minecraft:multi_action\",title:{menu_title},pause:false,after_action:\"close\",actions:[]}}"
    )];
    for w in &tracked {
        let k = w.ordinal.expect("tracked");
        let o = &c.quests.content.quests[w.quest].objectives[w.objective];
        let title = tr(o.title().unwrap_or_default());
        let tooltip = o
            .hint()
            .map(|h| format!(",tooltip:{}", snbt_of(&tr(h))))
            .unwrap_or_default();
        let action =
            format!("action:{{type:\"minecraft:run_command\",command:\"trigger {PICK} set {k}\"}}");
        let marked = json!({ "text": "◆ ", "extra": [title.clone()] });
        menu.push(format!(
            "execute if score @s {TRACK} matches {k}{} run data modify storage {ns}:guide menu.dialog.actions append value {{label:{}{tooltip},{action}}}",
            w.guard,
            snbt_of(&marked)
        ));
        menu.push(format!(
            "execute unless score @s {TRACK} matches {k}{} run data modify storage {ns}:guide menu.dialog.actions append value {{label:{}{tooltip},{action}}}",
            w.guard,
            snbt_of(&title)
        ));
    }
    menu.push(format!(
        "execute if data storage {ns}:guide menu.dialog.actions[0] run function {ns}:compass_menu_show with storage {ns}:guide menu"
    ));
    menu.push(format!(
        "execute unless data storage {ns}:guide menu.dialog.actions[0] run tellraw @s {}",
        tr_with(
            &chrome.get(delvewright_dsl::chrome::COMPASS_NOTHING),
            &[("color", json!("gray"))]
        )
    ));
    fns.push(("compass_menu".to_string(), lines(&menu)));
    fns.push((
        "compass_menu_show".to_string(),
        lines(&["$dialog show @s $(dialog)".to_string()]),
    ));

    // compass_pick: the button's trigger names the objective to follow; the
    // line is told whether or not it moved, and the mark moves in the journal.
    let mut pick = vec![
        format!("scoreboard players operation @s {TRACK} = @s {PICK}"),
        format!("scoreboard players set @s {PICK} 0"),
        format!("scoreboard players set @s {TOLD} -1"),
        format!("function {ns}:compass_refresh"),
    ];
    if g.journal {
        pick.push(format!("function {ns}:journal_rewrite"));
    }
    fns.push(("compass_pick".to_string(), lines(&pick)));
    fns
}

/// The aim at a followed `kill` (spec-0104 §6.3), run as the player holding
/// the compass: the living body carrying the wave's `tag` nearest that player
/// (`@e` selects only living entities, so a body in its death animation is
/// never picked: `docs/reference/compiler.md`, the `compass` row), its
/// position read into the macro's storage; while no body of the wave is
/// alive, the wave's `anchor` (spin when it has none).
pub(super) fn wave_aim_lines(ns: &str, tag: &str, anchor: Option<[i32; 3]>) -> Vec<String> {
    let alive = format!("@e[tag={tag}]");
    let picked = format!("@e[tag={WAVE_AIM_TAG},limit=1]");
    let mut v = vec![
        format!("tag @e[tag={WAVE_AIM_TAG}] remove {WAVE_AIM_TAG}"),
        format!(
            "execute at @s run tag {} add {WAVE_AIM_TAG}",
            alive.replace(']', ",sort=nearest,limit=1]")
        ),
    ];
    for (i, axis) in ["x", "y", "z"].iter().enumerate() {
        v.push(format!(
            "execute store result storage {ns}:guide aim.{axis} int 1 run data get entity {picked} Pos[{i}]"
        ));
    }
    v.push(format!(
        "execute if entity {picked} run function {ns}:compass_aim with storage {ns}:guide aim"
    ));
    v.push(match anchor {
        Some(p) => format!(
            "execute unless entity {picked} run function {ns}:compass_aim {{x:{},y:{},z:{}}}",
            p[0], p[1], p[2]
        ),
        None => format!("execute unless entity {picked} run function {ns}:compass_spin"),
    });
    v.push(format!("tag @e[tag={WAVE_AIM_TAG}] remove {WAVE_AIM_TAG}"));
    v
}

/// The journal's item modifiers (§8.3): `(name, modifier)`.
pub(super) fn item_modifiers(
    g: &Guide,
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
) -> Vec<(String, Value)> {
    use delvewright_dsl::chrome as ch;
    let c = plan.campaign;
    let mut out = Vec::new();
    if !g.journal {
        return out;
    }
    let gap = || json!({ "text": "\n\n" });
    // The compass page's sentences follow one another without a blank line, so
    // the page fits a book's fourteen lines in every language.
    let line = || json!({ "text": "\n" });
    let mut prefix: Vec<Value> = Vec::new();
    if g.compass {
        prefix.push(json!({ "text": "", "extra": [
            tr_with(&chrome.get(ch::JOURNAL_COMPASS_HEADING), &[("bold", json!(true))]),
            gap(),
            tr(&chrome.get(ch::JOURNAL_COMPASS_FOLLOWS)),
            line(),
            tr_with(&chrome.get(ch::JOURNAL_COMPASS_SWITCH), &[("with", json!([{ "keybind": "key.use" }]))]),
            line(),
            tr(&chrome.get(ch::JOURNAL_COMPASS_SPIN)),
            line(),
            tr(&chrome.get(ch::JOURNAL_COMPASS_PAGES)),
        ]}));
    }
    if let Some(m) = &c.quests.content.guidance.manual {
        prefix.extend(m.pages.iter().map(|p| tr(p)));
    }
    out.push((
        "journal_prefix".to_string(),
        json!({ "function": "minecraft:set_written_book_pages", "mode": "replace_all", "pages": prefix }),
    ));
    for w in g.announced() {
        let o = &c.quests.content.quests[w.quest].objectives[w.objective];
        let title = o.title().unwrap_or_default();
        let mut open = vec![
            tr_with(
                &chrome.get(ch::JOURNAL_OPEN),
                &[("color", json!("dark_gray"))],
            ),
            gap(),
            tr_with(title, &[("bold", json!(true))]),
        ];
        if let Some(hint) = o.hint() {
            open.push(gap());
            open.push(tr(hint));
        }
        let page = |extra: &[Value]| json!({ "text": "", "extra": extra });
        out.push((
            format!("journal_open_{}", w.safe),
            json!({ "function": "minecraft:set_written_book_pages", "mode": "append", "pages": [page(&open)] }),
        ));
        if w.ordinal.is_some() {
            let mut following = open.clone();
            following.push(gap());
            following.push(tr_with(
                &chrome.get(ch::JOURNAL_FOLLOWING),
                &[("color", json!("dark_aqua"))],
            ));
            out.push((
                format!("journal_open_following_{}", w.safe),
                json!({ "function": "minecraft:set_written_book_pages", "mode": "append", "pages": [page(&following)] }),
            ));
        }
        let done = [
            tr_with(
                &chrome.get(ch::JOURNAL_DONE),
                &[("color", json!("dark_gray"))],
            ),
            gap(),
            tr_with(title, &[("strikethrough", json!(true))]),
        ];
        out.push((
            format!("journal_done_{}", w.safe),
            json!({ "function": "minecraft:set_written_book_pages", "mode": "append", "pages": [page(&done)] }),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The coordinates a summon line is written at: its three tokens after
    /// the entity id.
    fn at(line: &str) -> [&str; 3] {
        let t: Vec<&str> = line.split(' ').collect();
        [t[2], t[3], t[4]]
    }

    /// A block display draws its block from its origin as the minimum corner,
    /// so it is summoned at the lit cell's corner, every axis written with a
    /// decimal point so the summon cannot centre it — on either side of zero.
    #[test]
    fn a_block_display_stands_at_its_cells_corner() {
        for (cell, want) in [
            ([8209, 64, 8192], ["8209.0", "64.0", "8192.0"]),
            ([-16, -3, -1], ["-16.0", "-3.0", "-1.0"]),
            ([0, 0, 0], ["0.0", "0.0", "0.0"]),
        ] {
            let line = display_summon("o_x", 0, cell, "minecraft:lever[face=floor,facing=north]");
            assert!(
                line.starts_with("summon minecraft:block_display "),
                "{line}"
            );
            assert_eq!(at(&line), want, "{line}");
        }
    }

    /// An item display draws its item centred on its origin, so it is
    /// summoned at the lit cell's centre on all three axes; a negative cell
    /// `-16` is centred on `-15.5`, never `-16.5`.
    #[test]
    fn an_item_display_stands_at_its_cells_centre() {
        for (cell, want) in [
            ([8209, 64, 8192], ["8209.5", "64.5", "8192.5"]),
            ([-16, -3, -1], ["-15.5", "-2.5", "-0.5"]),
        ] {
            let line = display_summon(
                "o_x",
                0,
                cell,
                "minecraft:bell[attachment=floor,facing=north]",
            );
            assert!(line.starts_with("summon minecraft:item_display "), "{line}");
            assert_eq!(at(&line), want, "{line}");
        }
    }

    /// A display draws the complete state: a lever written without
    /// `powered` is drawn unpowered, and that is the state its cell is held
    /// to, so a pulled lever no longer matches.
    #[test]
    fn a_display_draws_and_holds_the_complete_state() {
        assert_eq!(
            drawn_state("lever[face=floor,facing=north]"),
            "minecraft:lever[face=floor,facing=north,powered=false]"
        );
        assert_eq!(drawn_state("minecraft:stone"), "minecraft:stone");
        // An item display draws the item turned by `facing`: that is all it
        // holds its cell to.
        assert_eq!(
            drawn_predicate(&drawn_state("minecraft:chest")),
            "minecraft:chest[facing=north]"
        );
    }

    /// The one place a block highlight's display is summoned holds each cell
    /// to what it draws: killed where the cell differs, summoned where it
    /// matches and none stands.
    #[test]
    fn a_held_display_goes_when_its_cell_changes() {
        let state = drawn_state("minecraft:lever[face=floor,facing=north]");
        let h = HlAct {
            safe: "t_x".to_string(),
            objective: None,
            trigger: Some("trigger/x".to_string()),
            within: 6,
            body: HlBody::Blocks {
                cells: vec![([1, 2, 3], state.clone())],
                at: [1, 2, 3],
                staged: false,
                lit: None,
            },
            armed: String::new(),
        };
        let v = hold_lines(&h);
        assert_eq!(v.len(), 2, "{v:?}");
        assert_eq!(
            v[0],
            format!("execute unless block 1 2 3 {state} run kill @e[tag=dw_hl_t_x.0]")
        );
        assert!(
            v[1].starts_with(&format!(
                "execute if block 1 2 3 {state} unless entity @e[tag=dw_hl_t_x.0] run "
            )),
            "{}",
            v[1]
        );
        assert!(
            v[1].contains("run summon minecraft:block_display 1.0 2.0 3.0 {Glowing:1b,"),
            "{}",
            v[1]
        );
    }
}

/// `validation/journal-starts.json`: what the journal owes at every quest
/// start, for the bot to read off the book it holds (spec-0104 §8).
///
/// For each quest, the objectives the party is told of the moment it starts:
/// every announced objective with a hint and no `after`, whose gate is empty or
/// that a press `serves` (spec-0117 §4) — the presses are its first act, so its
/// page is owed while they are open. An objective with no `after` behind a gate
/// nothing serves is a thread not yet unlocked (spec-0104 §8): it is owed no
/// page and is listed under `gated`, counted, never judged. `active` is the
/// quest's activation score and each objective's `done` its completion score,
/// so the bot reads both and derives nothing.
pub(super) fn journal_ledger(plan: &Plan, g: &Guide) -> Option<Value> {
    if !g.journal {
        return None;
    }
    let c = plan.campaign;
    let guidance = &c.quests.content.guidance;
    let quests: Vec<Value> = c
        .quests
        .content
        .quests
        .iter()
        .map(|q| {
            let mut told = Vec::new();
            let mut gated = Vec::new();
            for o in q.objectives.iter().filter(|o| o.after().is_empty()) {
                let Some(hint) = o.hint().filter(|_| o.announced(guidance)) else {
                    continue;
                };
                let key = delvewright_dsl::l10n::untag(hint)
                    .map(|(k, _)| k.to_string())
                    .unwrap_or_default();
                let english = delvewright_dsl::l10n::untag(hint)
                    .map(|(_, e)| e.to_string())
                    .unwrap_or_else(|| hint.to_string());
                if gate_cond(plan, o.gate()).is_empty() || is_served(plan, o) {
                    told.push(json!({
                        "objective": o.id().as_str(),
                        "done": obj_score(o.id().as_str()),
                        "hint_key": key,
                        "hint": english,
                    }));
                } else {
                    gated.push(json!(o.id().as_str()));
                }
            }
            json!({
                "quest": q.id.as_str(),
                "active": quest_active_score(q.id.as_str()),
                "told": told,
                "gated": gated,
            })
        })
        .collect();
    Some(json!({ "party": plan::PARTY, "quests": quests }))
}
