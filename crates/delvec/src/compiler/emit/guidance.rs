//! Guidance a player can follow (spec-0104 §6–§8): the compass each player
//! aims at an open objective, the proximity highlight, and the journal.
//!
//! One derivation ([`Guide`]) is read by every emitter here, so the compass,
//! the journal and the highlight can never disagree about which objectives are
//! tracked, which are announced, or what a highlight lights.
//!
//! **When anything is rewritten.** The tick keeps one state per guided
//! objective — `0` closed, `1` pending (its `pending_guard` holds), `2`
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
/// Per player: `1` while the followed objective is a `talk-to`, whose target
/// walks.
const ON_BODY: &str = "dw.tbody";
/// Per player: deaths since the last refresh (`deathCount`).
const DEATHS: &str = "dw.gdeath";
/// The tag a player carries once their guidance entries are seeded.
const SEEDED_TAG: &str = "dw_guide_seeded";
/// The campaign's guidance epoch holder.
const EPOCH: &str = "#epoch";
/// The twenty-tick clock that re-aims a compass following a body.
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
    /// Its `pending_guard`, a run of ` if …`/` unless …` clauses.
    pub guard: String,
    /// Announced: listed in the journal.
    pub announced: bool,
    /// Tracked: its ordinal on the compass, from 1, in document order.
    pub ordinal: Option<usize>,
    /// Where the compass points: a cell, or a body's tag (a `talk-to`'s NPC).
    pub target: Option<Target>,
}

/// Where the compass points for one objective (§6.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Target {
    /// A fixed cell.
    Cell([i32; 3]),
    /// A body's entity tag; read live.
    Body(String),
}

/// What one highlight lights (§7.2).
#[derive(Clone, Debug)]
pub(super) enum HlBody {
    /// Block displays: one per cell, with the state each draws, polled from
    /// `at`.
    Blocks {
        cells: Vec<([i32; 3], String)>,
        at: [i32; 3],
    },
    /// Every body carrying this tag (a wave), or the NPC's body.
    Bodies { tag: String, npc: bool },
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
/// the state of a block a highlight lights that the campaign did not place.
pub(super) fn guide(plan: &Plan, blocks: &crate::compiler::blockstate::BlockMap) -> Guide {
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
                guard: pending_guard(plan, o, &qa),
                announced,
                ordinal: ord,
                target,
            });
        }
    }
    g.compass = ordinal > 0;
    g.journal = g.watched.iter().any(|w| w.announced) || guidance.manual.is_some();
    g.highlights = highlight_acts(plan, blocks, &g);
    g
}

fn highlight_acts(
    plan: &Plan,
    blocks: &crate::compiler::blockstate::BlockMap,
    g: &Guide,
) -> Vec<HlAct> {
    use crate::compiler::pressable::Body;
    use delvewright_dsl::TriggerOn;
    let c = plan.campaign;
    let block_at = |cell: [i32; 3]| -> Option<String> {
        blocks
            .get(&cell)
            .map(|b| b.as_str().to_string())
            .filter(|b| !delvewright_dsl::blockshape::is_air(b))
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
            Objective::Interact { anchor, prop, .. } => {
                let Some(cell) = at(anchor.as_str()) else {
                    continue;
                };
                let state = prop
                    .as_ref()
                    .map(|p| namespaced(&p.block))
                    .or_else(|| block_at(cell));
                let Some(state) = state else { continue };
                HlBody::Blocks {
                    cells: vec![(cell, state)],
                    at: cell,
                }
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
                let (cell, state) = match adopted {
                    Some(cell) => match block_at(cell) {
                        Some(s) => (cell, s),
                        None => continue,
                    },
                    None => match at(anchor.as_str()) {
                        Some(cell) => (cell, "minecraft:chest".to_string()),
                        None => continue,
                    },
                };
                HlBody::Blocks {
                    cells: vec![(cell, state)],
                    at: cell,
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
                let cells: Vec<([i32; 3], String)> =
                    match crate::compiler::pressable::trigger_body(plan, t) {
                        Body::Block { cell, block } => vec![(cell, namespaced(&block))],
                        Body::Point(cell) => match t
                            .prop
                            .as_ref()
                            .map(|p| namespaced(&p.block))
                            .or_else(|| block_at(cell))
                        {
                            Some(s) => vec![(cell, s)],
                            None => Vec::new(),
                        },
                        Body::Region(cells) => cells
                            .iter()
                            .filter_map(|c| block_at(*c).map(|s| (*c, s)))
                            .collect(),
                        Body::Rides { .. } => match t.at_anchor().and_then(|a| {
                            plan.anchors.iter().find_map(|((_, n), r)| match r {
                                ResolvedAnchor::Gate { from, to, .. } if n == a => {
                                    Some((*from, *to))
                                }
                                _ => None,
                            })
                        }) {
                            Some((from, to)) => {
                                let mut v = Vec::new();
                                for x in from[0].min(to[0])..=from[0].max(to[0]) {
                                    for y in from[1].min(to[1])..=from[1].max(to[1]) {
                                        for z in from[2].min(to[2])..=from[2].max(to[2]) {
                                            if let Some(s) = block_at([x, y, z]) {
                                                v.push(([x, y, z], s));
                                            }
                                        }
                                    }
                                }
                                v
                            }
                            None => Vec::new(),
                        },
                        Body::Nothing => Vec::new(),
                    };
                let Some(at) = cells.first().map(|c| c.0) else {
                    continue;
                };
                HlBody::Blocks { cells, at }
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
    out
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

/// The summon of one highlight display at `cell` drawing `state` (§7.2).
pub(super) fn display_summon(safe: &str, cell: [i32; 3], state: &str) -> String {
    let base = state.split('[').next().unwrap_or(state);
    let tags = format!("Tags:[{FIXTURE_NBT}\"dw_hl\",\"dw_hl_{safe}\"]");
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

/// The lines that place a highlight's displays: run at an objective's
/// activation, and at `setup_finish` for a trigger. Guarded so a second run
/// places nothing twice. Empty for a body highlight.
pub(super) fn show_lines(h: &HlAct) -> Vec<String> {
    match &h.body {
        HlBody::Blocks { cells, .. } => {
            let mut v = vec![format!("kill @e[tag=dw_hl_{}]", h.safe)];
            v.extend(cells.iter().map(|(c, s)| display_summon(&h.safe, *c, s)));
            v
        }
        HlBody::Bodies { .. } => Vec::new(),
    }
}

/// The lines an objective's completion runs for its highlight: the displays
/// removed, the bodies' glow cleared.
pub(super) fn completion_lines(g: &Guide, oid: &str) -> Vec<String> {
    let Some(h) = g.highlight_of(oid) else {
        return Vec::new();
    };
    let mut v = vec![format!("scoreboard players set #hl_{} {GUIDE} 0", h.safe)];
    match &h.body {
        HlBody::Blocks { .. } => v.push(format!("kill @e[tag=dw_hl_{}]", h.safe)),
        HlBody::Bodies { tag, npc } => {
            let sel = body_selector(tag, *npc);
            v.push(format!("effect clear {sel} minecraft:glowing"));
            v.push(format!("team leave {sel}"));
        }
    }
    v
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

/// The `setup_finish` lines: a trigger's highlight displays.
pub(super) fn setup_finish_lines(g: &Guide) -> Vec<String> {
    g.highlights
        .iter()
        .filter(|h| h.trigger.is_some())
        .flat_map(show_lines)
        .collect()
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
            .any(|w| matches!(w.target, Some(Target::Body(_))))
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
            HlBody::Blocks { at, .. } => poll.push(format!(
                "execute {}positioned {} {} {} if entity {near} run scoreboard players set {hin} {GUIDE} 1",
                h.armed,
                center(at[0]),
                center(at[1]),
                center(at[2])
            )),
            HlBody::Bodies { tag, npc } => poll.push(format!(
                "execute {}as {} at @s if entity {near} run scoreboard players set {hin} {GUIDE} 1",
                h.armed,
                body_selector(tag, *npc)
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
        fns.push((format!("hl_poll_{}", h.safe), lines(&poll)));
        let (on, off) = match &h.body {
            HlBody::Blocks { .. } => (
                vec![format!(
                    "execute as @e[tag=dw_hl_{}] run data merge entity @s {{Glowing:1b}}",
                    h.safe
                )],
                vec![format!(
                    "execute as @e[tag=dw_hl_{}] run data merge entity @s {{Glowing:0b}}",
                    h.safe
                )],
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
        };
        let mut on = on;
        on.push(format!("scoreboard players set {hl} {GUIDE} 1"));
        let mut off = off;
        off.push(format!("scoreboard players set {hl} {GUIDE} 0"));
        fns.push((format!("hl_on_{}", h.safe), lines(&on)));
        fns.push((format!("hl_off_{}", h.safe), lines(&off)));
        if matches!(h.body, HlBody::Blocks { .. }) {
            fns.push((format!("hl_show_{}", h.safe), lines(&show_lines(h))));
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
            let line = display_summon("o_x", cell, "minecraft:lever[face=floor,facing=north]");
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
            let line = display_summon("o_x", cell, "minecraft:bell[attachment=floor,facing=north]");
            assert!(line.starts_with("summon minecraft:item_display "), "{line}");
            assert_eq!(at(&line), want, "{line}");
        }
    }
}
