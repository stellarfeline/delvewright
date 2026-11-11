//! **A story moment as a picture holds it** (spec-0089 §4): the world on a
//! path once its first `arrival` steps are played, in the order the path
//! plays them.
//!
//! Three things make the moment, and each is asked of its own authority:
//!
//! - **the blocks** — the load bytes with every write fired on the way laid
//!   in firing order ([`lay_in_order`]): the setup's writes at world init,
//!   then each step's region writes
//!   ([`crate::compiler::nav::Configuration::laid_writes`]) and the
//!   placements ledger's block rows ([`crate::compiler::placed`]) at the step
//!   that fires them, so a cell the setup lays and a later beat rewrites
//!   holds the beat's block;
//! - **the cast** — where the cast ledger stations every NPC it declares at
//!   that moment of that path ([`cast_at`]: [`crate::compiler::cast::station`]
//!   under the flags the path holds there, resolved through
//!   [`crate::compiler::plan::body_station`] as the critical path's `talk-to`
//!   steps resolve it), each body and hitbox moved there, taken out of the
//!   world when the ledger declares it absent, and summoned there when the
//!   path's ledger rows never summoned it;
//! - **the sky** — the latest `set-time` and `set-weather` the path's replay
//!   fired by then ([`clock_at`]).

use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::blockstate::BlockMap;
use crate::compiler::flow::JournalStep;
use crate::compiler::placed::{Placed, PlacedObject};
use crate::compiler::plan::{Plan, Step};
use crate::compiler::view::world::EntityRecord;

/// An inclusive box of cells, `(min, max)`.
pub type Region = ([i32; 3], [i32; 3]);

/// **Every write a moment holds, in firing order**: the ledger's setup rows
/// first (world init), then per step its region writes (`writes`: region,
/// block, firing step — `None` lays air) and then the ledger's block rows
/// fired at that step. Rows that are not blocks take no part.
#[derive(Clone)]
pub struct Laid {
    /// The writes in the order they are laid.
    pub ops: Vec<crate::compiler::nav::LaidWrite>,
    /// What the writes leave in the cells they reach, laid over nothing.
    over: BlockMap,
    /// Every cell a write reaches.
    cells: BTreeSet<[i32; 3]>,
}

impl Laid {
    /// The writes of a moment, ordered.
    pub fn of(writes: &[crate::compiler::nav::FiredWrite], rows: &[&Placed]) -> Laid {
        use crate::compiler::placed::Firing;
        // (time, kind, sequence): the setup at world init, then a step's
        // region writes, then the same step's ledger rows.
        let mut ops: Vec<((usize, u8, usize), crate::compiler::nav::LaidWrite)> = Vec::new();
        for (i, (r, b, step)) in writes.iter().enumerate() {
            ops.push(((*step, 1, i), (*r, b.clone())));
        }
        for (i, row) in rows.iter().enumerate() {
            let PlacedObject::Block { cell, state } = &row.object else {
                continue;
            };
            let key = match row.fired {
                Firing::Setup => (0, 0, i),
                Firing::Step(s) => (s, 2, i),
                Firing::Unforced(_) => continue,
            };
            let block = (crate::compiler::assembled::base_id(state) != "minecraft:air")
                .then(|| state.clone());
            ops.push((key, ((*cell, *cell), block)));
        }
        ops.sort_by_key(|(k, _)| *k);
        let ops: Vec<crate::compiler::nav::LaidWrite> = ops.into_iter().map(|(_, w)| w).collect();
        let mut cells = BTreeSet::new();
        for ((lo, hi), _) in &ops {
            cells.extend(crate::compiler::assembled::region_cells(*lo, *hi));
        }
        Laid {
            over: crate::compiler::nav::lay_writes(&BlockMap::new(), &ops),
            ops,
            cells,
        }
    }

    /// **The moment's bytes**: `base` with every write laid in order.
    pub fn bytes(&self, base: &BlockMap) -> BlockMap {
        crate::compiler::nav::lay_writes(base, &self.ops)
    }

    /// The regions written, in order: the world writer holds the level
    /// generator's ground out of them.
    pub fn regions(&self) -> Vec<Region> {
        self.ops.iter().map(|(r, _)| *r).collect()
    }

    /// The block this moment holds at `c` over `base`.
    fn at<'a>(&'a self, base: &'a BlockMap, c: &[i32; 3]) -> Option<&'a str> {
        if self.cells.contains(c) {
            self.over.get(c).map(|s| s.as_str())
        } else {
            base.get(c).map(|s| s.as_str())
        }
    }

    /// **How many cells this moment holds differently from `other`** over the
    /// same `base` — the count of cells [`Laid::bytes`] of the two differ in,
    /// asked only of the cells either one writes.
    pub fn moved_from(&self, other: &Laid, base: &BlockMap) -> usize {
        self.cells
            .union(&other.cells)
            .filter(|c| self.at(base, c) != other.at(base, c))
            .count()
    }
}

/// Where the cast ledger stands one NPC at a moment.
#[derive(Debug, Clone, PartialEq)]
pub struct CastMark {
    /// The NPC's id (`npc/…`).
    pub npc: String,
    /// The tag its body and its hitbox both carry (`dw_npc_<n>`).
    pub tag: String,
    /// Its feet and yaw, or `None` while the ledger declares it out of the
    /// world.
    pub at: Option<([f64; 3], f32)>,
}

/// What a path holds at a moment, as the cast ledger is asked it: the quest
/// being played, the quests begun, and the flags held.
pub struct StoryAt {
    pub quest: String,
    pub begun: BTreeSet<String>,
    pub flags: BTreeSet<String>,
}

/// **The story at arrival `arrival` of a path** whose steps are `steps` and
/// whose flow journal is `journal`: the objectives of `steps[..arrival]` are
/// played; the flags are those held once the last of them fired; the quest
/// is the one the next unplayed objective belongs to (the last played one's
/// past the end); the quests begun are those the played steps opened and the
/// quest of the moment. `None` for a path with no journalled objective.
pub fn story_at(steps: &[Step], journal: &[JournalStep], arrival: usize) -> Option<StoryAt> {
    let played: BTreeSet<&str> = steps[..arrival.min(steps.len())]
        .iter()
        .filter_map(Step::objective)
        .collect();
    let last = journal
        .iter()
        .rposition(|j| played.contains(j.objective.as_str()));
    let next = journal
        .iter()
        .find(|j| !played.contains(j.objective.as_str()));
    let quest = next
        .or_else(|| last.map(|i| &journal[i]))
        .map(|j| j.quest.clone())?;
    let flags = match last {
        Some(i) => journal[i].flags_after.clone(),
        None => journal
            .first()
            .map(|j| j.flags_before.clone())
            .unwrap_or_default(),
    };
    let mut begun: BTreeSet<String> = BTreeSet::new();
    for j in journal.iter().take(last.map_or(0, |i| i + 1)) {
        begun.insert(j.quest.clone());
        begun.extend(j.opened.iter().cloned());
    }
    begun.insert(quest.clone());
    Some(StoryAt {
        quest,
        begun,
        flags,
    })
}

/// **The cast at a moment**: one [`CastMark`] per NPC the cast ledger
/// declares up to the moment's quest, in stage-2 order — the station
/// [`crate::compiler::cast::station`] gives under the moment's flags, its
/// anchor resolved in the beat's area first and the NPC's home second
/// ([`crate::compiler::plan::BodyScope::Beat`]), plus the row's offset. A body
/// a planned walk (`moves`) leaves on that mark stands where the walk ends,
/// facing its last step; any other stands on the cell's centre, facing its
/// home facing on its home cell and its anchor's facing elsewhere. An NPC no
/// row declares, or whose anchor does not resolve, is left as the ledger rows
/// place it (absent from the result).
pub fn cast_at(
    plan: &Plan,
    story: &StoryAt,
    moves: &[crate::compiler::nav::MovePlan],
) -> Vec<CastMark> {
    use crate::compiler::cast::Station;
    use crate::compiler::plan::{BodyScope, BodyStation, ResolvedAnchor, body_station};
    let c = plan.campaign;
    let beat_area = c
        .quest_plan
        .content
        .quests
        .iter()
        .find(|q| q.id.as_str() == story.quest)
        .map(|q| q.area.as_str())
        .unwrap_or("");
    let mut out = Vec::new();
    for npc in &c.npcs.content.npcs {
        let id = npc.id.as_str();
        let tag = format!("dw_npc_{}", crate::compiler::plan::safe_local(id));
        let Some(station) =
            crate::compiler::cast::station(c, id, &story.quest, &story.begun, &story.flags)
        else {
            continue;
        };
        let at = match station {
            Station::Absent(_) => None,
            Station::At(anchor, offset) => {
                let st = body_station(
                    &plan.anchors,
                    BodyScope::Beat {
                        beat: beat_area,
                        home: npc.area.as_str(),
                    },
                    anchor,
                );
                let BodyStation::At {
                    anchor: resolved, ..
                } = &st
                else {
                    continue;
                };
                let (pos, facing) = match resolved {
                    ResolvedAnchor::Point { pos, facing } => (*pos, facing.as_deref()),
                    ResolvedAnchor::Gate { from, .. } => (*from, None),
                };
                let cell = delvewright_dsl::offset_cell(pos, offset);
                let (home, home_yaw) = crate::compiler::watching::npc_home(plan, npc);
                let walked = moves
                    .iter()
                    .filter(|m| m.npc == id && m.mark_cell == cell)
                    .find_map(|m| Some((*m.waypoints.last()?, *m.yaws.last()? as f32)));
                Some(match walked {
                    Some(w) => w,
                    None => {
                        let yaw = if cell == home {
                            home_yaw
                        } else {
                            crate::compiler::emit::facing_yaw(facing)
                        };
                        (crate::compiler::nav::cell_center(cell), yaw as f32)
                    }
                })
            }
        };
        out.push(CastMark {
            npc: id.to_string(),
            tag,
            at,
        });
    }
    out
}

/// **The bodies a moment's world holds**: the ledger rows' bodies, with every
/// NPC the cast declares (`cast`) stood where it stands — its body and its
/// hitbox moved to the mark, taken out while it is absent, and summoned from
/// its rows anywhere in `templates` (a `spawn-npc` the path's rows do not
/// fire) when the rows hold no body of it.
pub fn bodies_of(rows: &[&Placed], cast: &[CastMark], templates: &[Placed]) -> Vec<EntityRecord> {
    let held: Vec<EntityRecord> = rows.iter().filter_map(|r| r.entity_record()).collect();
    let tagged = |e: &EntityRecord, tag: &str| e.tags.iter().any(|t| t == tag);
    let mut out: Vec<EntityRecord> = held
        .iter()
        .filter(|e| !cast.iter().any(|m| tagged(e, &m.tag)))
        .cloned()
        .collect();
    for m in cast {
        let Some((feet, yaw)) = m.at else { continue };
        let mut mine: Vec<EntityRecord> =
            held.iter().filter(|e| tagged(e, &m.tag)).cloned().collect();
        if mine.is_empty() {
            // The body and its hitbox as the first rows that summon them.
            let all: Vec<EntityRecord> = templates
                .iter()
                .filter_map(Placed::entity_record)
                .filter(|e| tagged(e, &m.tag))
                .collect();
            let body = all.iter().find(|e| tagged(e, "dw_npc")).cloned();
            let hitbox = all.iter().find(|e| !tagged(e, "dw_npc")).cloned();
            mine.extend(body);
            mine.extend(hitbox);
        }
        for mut e in mine {
            e.pos = feet;
            if tagged(&e, "dw_npc") {
                e.yaw = yaw;
            }
            out.push(e);
        }
    }
    out
}

/// **The sky a moment holds**: the latest `set-time` and the latest
/// `set-weather` among the effects the path's replay fired at the played
/// objectives (`journal`'s `fired`, in journal order), or `None` for each the
/// path has not set — the world's own.
pub fn clock_at(
    plan: &Plan,
    steps: &[Step],
    journal: &[JournalStep],
    arrival: usize,
) -> (Option<String>, Option<String>) {
    use delvewright_dsl::Verb;
    let played: BTreeSet<&str> = steps[..arrival.min(steps.len())]
        .iter()
        .filter_map(Step::objective)
        .collect();
    let mut by_path: BTreeMap<String, (Option<String>, Option<String>)> = BTreeMap::new();
    crate::compiler::plan::for_each_gate_effect(plan.campaign, &mut |site, e| match &e.verb {
        Verb::SetTime { time, .. } => {
            by_path.insert(site.path.clone(), (Some(format!("{time:?}")), None));
        }
        Verb::SetWeather { weather, .. } => {
            by_path.insert(site.path.clone(), (None, Some(weather.token().to_string())));
        }
        _ => {}
    });
    let (mut time, mut weather) = (None, None);
    for j in journal
        .iter()
        .filter(|j| played.contains(j.objective.as_str()))
    {
        for p in &j.fired_in_order {
            if let Some((t, w)) = by_path.get(p) {
                if t.is_some() {
                    time = t.clone();
                }
                if w.is_some() {
                    weather = w.clone();
                }
            }
        }
    }
    (time, weather)
}

/// **The bodies as a picture taken from `lens` shows them** (spec-0101): a
/// player standing with their eyes at the lens is who a body that watches the
/// nearest player turns to, so every body carrying one of `watchers`' tags
/// (`(tag, reach)`) whose feet are within its reach of that player's feet
/// faces the lens, as the game's look-at turns it (yaw from
/// [`crate::compiler::watching::bearing`]); its pitch is the record's own.
/// Returns how many bodies turned.
pub fn facing_the_lens(
    bodies: &mut [EntityRecord],
    watchers: &[(String, u32)],
    lens: [f64; 3],
) -> usize {
    let feet = [
        lens[0],
        lens[1] - delvewright_dsl::metrics::PLAYER_EYE_HEIGHT,
        lens[2],
    ];
    let mut turned = 0;
    for e in bodies.iter_mut() {
        if !e
            .tags
            .iter()
            .any(|t| t == crate::compiler::watching::WATCH_TAG)
        {
            continue;
        }
        let Some((_, reach)) = watchers
            .iter()
            .find(|(tag, _)| e.tags.iter().any(|t| t == tag))
        else {
            continue;
        };
        let d = (0..3)
            .map(|i| (e.pos[i] - feet[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        if d > f64::from(*reach) {
            continue;
        }
        e.yaw = crate::compiler::watching::bearing(e.pos, lens) as f32;
        turned += 1;
    }
    turned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::placed::Firing;

    /// **Every write in firing order**: the setup lays stone at a cell, and
    /// the beat at step 3 lays gold over it — a region write and a ledger row
    /// alike; the moment holds the gold, and two moments' moved count is the
    /// count of cells their bytes differ in.
    #[test]
    fn a_beat_s_write_lands_over_the_setup_s_and_the_count_is_the_bytes() {
        let cell = [1, 65, 1];
        let setup = Placed::block(Firing::Setup, cell, "minecraft:stone");
        let beat_row = Placed::block(Firing::Step(3), [2, 65, 1], "minecraft:gold_block");
        let base = BlockMap::new();
        let load = Laid::of(&[], &[&setup]);
        let writes = [((cell, cell), Some("minecraft:gold_block".to_string()), 3)];
        // The rows handed in their ledger order, setup last: firing order
        // decides, not the order handed.
        let after = Laid::of(&writes, &[&beat_row, &setup]);
        let bytes = after.bytes(&base);
        assert_eq!(
            bytes.get(&cell).map(|s| s.as_str()),
            Some("minecraft:gold_block")
        );
        assert_eq!(
            bytes.get(&[2, 65, 1]).map(|s| s.as_str()),
            Some("minecraft:gold_block")
        );
        let lb = load.bytes(&base);
        let differ = [cell, [2, 65, 1]]
            .iter()
            .filter(|c| lb.get(c).map(|s| s.as_str()) != bytes.get(c).map(|s| s.as_str()))
            .count();
        assert_eq!(after.moved_from(&load, &base), differ);
        assert_eq!(differ, 2);
        // An unforced row is never laid.
        let unforced = Placed::block(Firing::Unforced("a trap".into()), cell, "minecraft:dirt");
        let none = Laid::of(&[], &[&unforced]);
        assert_eq!(none.bytes(&base).get(&cell), None);
    }

    /// **A watcher faces the lens it is within reach of**, and only that one.
    #[test]
    fn a_watcher_within_reach_turns_to_the_lens() {
        let body = |tag: &str, pos: [f64; 3]| EntityRecord {
            kind: "minecraft:villager".into(),
            pos,
            yaw: 180.0,
            tags: vec![
                "dw_npc".into(),
                tag.into(),
                crate::compiler::watching::WATCH_TAG.into(),
            ],
            nbt: crate::compiler::view::world::Nbt::Compound(Vec::new()),
        };
        let mut bodies = vec![
            body("dw_npc_near", [0.5, 64.0, 4.5]),
            body("dw_npc_far", [0.5, 64.0, 40.5]),
            body("dw_npc_none", [1.5, 64.0, 4.5]),
        ];
        let watchers = vec![
            ("dw_npc_near".to_string(), 8),
            ("dw_npc_far".to_string(), 8),
        ];
        let lens = [0.5, 65.62, 0.5];
        assert_eq!(facing_the_lens(&mut bodies, &watchers, lens), 1);
        // South of the lens, the body turns north to it.
        assert!(
            (f64::from(bodies[0].yaw) - 180.0).abs() < 1e-3
                || (f64::from(bodies[0].yaw) + 180.0).abs() < 1e-3
        );
        assert_eq!(bodies[1].yaw, 180.0, "beyond its reach");
        assert_eq!(bodies[2].yaw, 180.0, "not a watcher");
        let mut east = vec![body("dw_npc_near", [4.5, 64.0, 0.5])];
        facing_the_lens(&mut east, &watchers, lens);
        assert!(
            (f64::from(east[0].yaw) - 90.0).abs() < 1e-3,
            "{}",
            east[0].yaw
        );
    }
}
