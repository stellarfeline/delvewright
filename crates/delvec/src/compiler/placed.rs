//! **The placements ledger** (spec-0112 §4): every block, block entity and body
//! the datapack places outside the structure templates, and every removal, as
//! one record each — and the command that does it is written from the record.
//!
//! A [`Placed`] row is the object (a block at a cell, a container's item, a
//! body with its whole summon compound, a removal by tag) and when it fires: at
//! world setup, at a critical-path step, or on a beat nobody has to play. The
//! emitter makes every placing line of the functions this module names through
//! [`Placed::command`], so the line and the record cannot disagree; the build
//! writes the ledger as `validation/placed.json`, and a configuration's world
//! ([`crate::compiler::view::review`]) lays every row its arrival holds
//! ([`rows_at`]). The region writes stay [`crate::compiler::plan::RegionEvent`]:
//! a row is a single cell or a single body, a region event is a box.
//!
//! `nbt` is the summon's whole compound as the command carries it — a
//! mannequin's `profile`, a `block_display`'s `block_state` and
//! `transformation`, an `item_display`'s `item` — parsed back by [`parse_snbt`]
//! into the compound the written world holds, so the body written is the body
//! the server would hold.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::compiler::view::world::{EntityRecord, Nbt};

/// When a row's command runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Firing {
    /// In `setup_finish`, before any beat.
    Setup,
    /// At the completion, activation or performance of `critical_path[i]`.
    Step(usize),
    /// On a beat nobody has to play (a sprung trap, an optional quest), named:
    /// no configuration lays it.
    Unforced(String),
}

/// What a row places.
#[derive(Debug, Clone, PartialEq)]
pub enum PlacedObject {
    /// `setblock <x> <y> <z> <state>`.
    Block { cell: [i32; 3], state: String },
    /// `item replace block <x> <y> <z> <slot> with <stack>`: a container's item.
    Item {
        cell: [i32; 3],
        slot: String,
        stack: String,
    },
    /// `summon <kind> <x> <y> <z> <nbt>`: a body, a hitbox, a marker, an
    /// assembly part. `pos` is the command's own coordinate text.
    Entity {
        kind: String,
        pos: [String; 3],
        nbt: String,
    },
    /// `kill @e[tag=<tag>]`, or `<prefix> kill @e[tag=<tag>]`: every earlier
    /// body carrying the tag is removed.
    Removed { tag: String, line: String },
}

/// One row of the ledger.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub fired: Firing,
    pub object: PlacedObject,
    /// The function whose line this row is; empty for a row no function has
    /// been read for yet.
    pub function: String,
}

impl Placed {
    /// A block written at setup or at a firing.
    pub fn block(fired: Firing, cell: [i32; 3], state: &str) -> Placed {
        Placed {
            fired,
            object: PlacedObject::Block {
                cell,
                state: state.to_string(),
            },
            function: String::new(),
        }
    }

    /// A body summoned at `pos` (the command's coordinate text) with its
    /// whole compound `nbt` (`{…}`).
    pub fn entity(fired: Firing, kind: &str, pos: [String; 3], nbt: String) -> Placed {
        Placed {
            fired,
            object: PlacedObject::Entity {
                kind: kind.to_string(),
                pos,
                nbt,
            },
            function: String::new(),
        }
    }

    /// The row a `summon <kind> <x> <y> <z> <nbt>` line states. `None` for any
    /// other line.
    pub fn of_summon(fired: Firing, line: &str) -> Option<Placed> {
        let rest = line.strip_prefix("summon ")?;
        let mut it = rest.splitn(5, ' ');
        let kind = it.next()?;
        let x = it.next()?;
        let y = it.next()?;
        let z = it.next()?;
        let nbt = it.next().unwrap_or("{}");
        Some(Placed::entity(
            fired,
            kind,
            [x.to_string(), y.to_string(), z.to_string()],
            nbt.to_string(),
        ))
    }

    /// The row a `setblock <x> <y> <z> <state>` or `item replace block <x> <y>
    /// <z> <slot> with <stack>` line states. `None` for any other line.
    pub fn of_block_line(fired: Firing, line: &str) -> Option<Placed> {
        if let Some(rest) = line.strip_prefix("setblock ") {
            let mut it = rest.splitn(4, ' ');
            let c = [
                it.next()?.parse().ok()?,
                it.next()?.parse().ok()?,
                it.next()?.parse().ok()?,
            ];
            return Some(Placed::block(fired, c, it.next()?));
        }
        let rest = line.strip_prefix("item replace block ")?;
        let mut it = rest.splitn(4, ' ');
        let c = [
            it.next()?.parse().ok()?,
            it.next()?.parse().ok()?,
            it.next()?.parse().ok()?,
        ];
        let tail = it.next()?;
        let (slot, stack) = tail.split_once(" with ")?;
        Some(Placed {
            fired,
            object: PlacedObject::Item {
                cell: c,
                slot: slot.to_string(),
                stack: stack.to_string(),
            },
            function: String::new(),
        })
    }

    /// A removal of every body tagged `tag`, by the line `line`.
    pub fn removed(fired: Firing, tag: &str, line: &str) -> Placed {
        Placed {
            fired,
            object: PlacedObject::Removed {
                tag: tag.to_string(),
                line: line.to_string(),
            },
            function: String::new(),
        }
    }

    /// **The one way a placing line is written**: the command this row runs.
    pub fn command(&self) -> String {
        match &self.object {
            PlacedObject::Block { cell, state } => {
                format!("setblock {} {} {} {state}", cell[0], cell[1], cell[2])
            }
            PlacedObject::Item { cell, slot, stack } => format!(
                "item replace block {} {} {} {slot} with {stack}",
                cell[0], cell[1], cell[2]
            ),
            PlacedObject::Entity { kind, pos, nbt } => {
                format!("summon {kind} {} {} {} {nbt}", pos[0], pos[1], pos[2])
            }
            PlacedObject::Removed { line, .. } => line.clone(),
        }
    }

    /// The row as `validation/placed.json` states it.
    pub fn to_json(&self, plan: &crate::compiler::plan::Plan) -> Value {
        let fired = match &self.fired {
            Firing::Setup => json!("setup"),
            Firing::Step(i) => json!({
                "step": i,
                "id": plan.critical_path.get(*i).and_then(crate::compiler::view::beat::step_id),
            }),
            Firing::Unforced(blame) => json!({ "unforced": blame }),
        };
        let object = match &self.object {
            PlacedObject::Block { cell, state } => json!({"block": state, "cell": cell}),
            PlacedObject::Item { cell, slot, stack } => {
                json!({"item": stack, "slot": slot, "cell": cell})
            }
            PlacedObject::Entity { kind, pos, .. } => {
                let tags = self.entity_record().map(|e| e.tags).unwrap_or_default();
                json!({"entity": kind, "pos": pos, "tags": tags})
            }
            PlacedObject::Removed { tag, .. } => json!({"removed": tag}),
        };
        json!({
            "fired": fired,
            "function": self.function,
            "object": object,
            "command": self.command(),
        })
    }

    /// The body this row summons, as the written world holds it: the parsed
    /// compound, the numeric position, the yaw its `Rotation` states and its
    /// `Tags`. `None` for a row that summons nothing, or a position given
    /// relative to an executor (`~`), which no world can write.
    pub fn entity_record(&self) -> Option<EntityRecord> {
        let PlacedObject::Entity { kind, pos, nbt } = &self.object else {
            return None;
        };
        let p = [
            pos[0].parse::<f64>().ok()?,
            pos[1].parse::<f64>().ok()?,
            pos[2].parse::<f64>().ok()?,
        ];
        let compound = parse_snbt(nbt).unwrap_or(Nbt::Compound(Vec::new()));
        let yaw = match compound.get("Rotation") {
            Some(Nbt::List(r)) => match r.first() {
                Some(Nbt::Float(f)) => *f,
                Some(Nbt::Double(d)) => *d as f32,
                Some(Nbt::Int(i)) => *i as f32,
                _ => 0.0,
            },
            _ => 0.0,
        };
        let tags = match compound.get("Tags") {
            Some(Nbt::List(t)) => t
                .iter()
                .filter_map(|x| match x {
                    Nbt::Str(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let kind = if kind.contains(':') {
            kind.clone()
        } else {
            format!("minecraft:{kind}")
        };
        Some(EntityRecord {
            kind,
            pos: p,
            yaw,
            tags,
            nbt: compound,
        })
    }
}

/// The whole ledger of a build, in emission order.
#[derive(Debug, Clone, Default)]
pub struct Ledger {
    pub rows: Vec<Placed>,
}

impl Ledger {
    /// Record `rows` and hand back their commands, in order.
    pub fn record(&mut self, rows: Vec<Placed>) -> Vec<String> {
        let lines = rows.iter().map(Placed::command).collect();
        self.rows.extend(rows);
        lines
    }

    /// `validation/placed.json`: every row with its firing, its object and its
    /// command, and the counts by firing and by object.
    pub fn to_json(&self, plan: &crate::compiler::plan::Plan) -> Value {
        let mut by_object: BTreeMap<&str, usize> = BTreeMap::new();
        let mut by_firing: BTreeMap<&str, usize> = BTreeMap::new();
        for r in &self.rows {
            *by_object
                .entry(match r.object {
                    PlacedObject::Block { .. } => "block",
                    PlacedObject::Item { .. } => "item",
                    PlacedObject::Entity { .. } => "entity",
                    PlacedObject::Removed { .. } => "removed",
                })
                .or_default() += 1;
            *by_firing
                .entry(match r.fired {
                    Firing::Setup => "setup",
                    Firing::Step(_) => "step",
                    Firing::Unforced(_) => "unforced",
                })
                .or_default() += 1;
        }
        json!({
            "rows": self.rows.iter().map(|r| r.to_json(plan)).collect::<Vec<_>>(),
            "by_object": by_object,
            "by_firing": by_firing,
        })
    }
}

impl Ledger {
    /// The body rows a world cannot hold as written: a position that is no
    /// absolute number (a summon run `at` something), or a compound that does
    /// not read as SNBT. Each is named on the build's binding line; none is
    /// written into a frame's world.
    pub fn unplaceable(&self) -> Vec<&Placed> {
        self.rows
            .iter()
            .filter(|r| match &r.object {
                PlacedObject::Entity { pos, nbt, .. } => {
                    pos.iter().any(|p| p.parse::<f64>().is_err()) || parse_snbt(nbt).is_err()
                }
                _ => false,
            })
            .collect()
    }

    /// Read a ledger back from the `validation/placed.json` a build wrote:
    /// each row from its firing and its command. A row whose command no longer
    /// reads as a placing line is dropped.
    pub fn from_json(v: &Value) -> Ledger {
        let mut rows = Vec::new();
        for r in v["rows"].as_array().into_iter().flatten() {
            let fired = match &r["fired"] {
                Value::String(s) if s == "setup" => Firing::Setup,
                f if f.get("step").is_some() => {
                    Firing::Step(f["step"].as_u64().unwrap_or(0) as usize)
                }
                f => Firing::Unforced(f["unforced"].as_str().unwrap_or("").to_string()),
            };
            let Some(cmd) = r["command"].as_str() else {
                continue;
            };
            if let Some((_, mut row)) = row_of_line(&fired, cmd) {
                row.function = r["function"].as_str().unwrap_or("").to_string();
                rows.push(row);
            }
        }
        Ledger { rows }
    }
}

/// [`rows_at`] under a stated ancestry (`ancestor(step, arrival)`), for a
/// reader that holds the plan's ancestry rather than the plan.
pub fn rows_under<'a>(
    ledger: &'a Ledger,
    arrival: usize,
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Vec<&'a Placed> {
    let holds = |f: &Firing| match f {
        Firing::Setup => true,
        Firing::Step(i) => *i < arrival && ancestor(*i, arrival),
        Firing::Unforced(_) => false,
    };
    let order = |f: &Firing| match f {
        Firing::Setup => 0usize,
        Firing::Step(i) => i + 1,
        Firing::Unforced(_) => usize::MAX,
    };
    let mut held: Vec<(usize, usize, &Placed)> = ledger
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| holds(&r.fired))
        .map(|(i, r)| (order(&r.fired), i, r))
        .collect();
    held.sort_by_key(|(o, i, _)| (*o, *i));
    let mut out: Vec<&Placed> = Vec::new();
    for (_, _, r) in held {
        if let PlacedObject::Removed { tag, .. } = &r.object {
            out.retain(|x| {
                x.entity_record()
                    .is_none_or(|e| !e.tags.iter().any(|t| t == tag))
            });
            continue;
        }
        out.push(r);
    }
    out
}

/// **What a configuration holds of the ledger** (spec-0112 §4.3): every row
/// fired at setup or at a step that is a causal ancestor of `arrival`, applied
/// in firing order, a removal deleting every earlier body carrying its tag.
/// Unforced rows are never laid.
pub fn rows_at<'a>(
    plan: &crate::compiler::plan::Plan,
    ledger: &'a Ledger,
    arrival: usize,
) -> Vec<&'a Placed> {
    rows_under(ledger, arrival, &|i, a| plan.gate_fired_before(i, a))
}

// ------------------------------------------------------------- the ledger

/// The unseen-removal tag a story's removal hands a body to
/// (`compiler::emit`'s `UNSEEN_TAG`).
const UNSEEN_TAG: &str = "dw_unseen";

/// Split an `execute … run <command>` line into its prefix (`execute … run `)
/// and the command it runs; a bare command has an empty prefix.
fn split_run(line: &str) -> (&str, &str) {
    if line.starts_with("execute ")
        && let Some(i) = line.rfind(" run ")
    {
        let (p, c) = line.split_at(i + 5);
        return (p, c);
    }
    ("", line)
}

/// The tag a removal line removes the bodies of: `kill @e[tag=<t>]`, or the
/// unseen hand-off `execute as @e[tag=<t>] run data merge entity @s
/// {Tags:["dw_unseen"]…}` (the body leaves the world under it).
fn removal_tag(line: &str) -> Option<String> {
    if let Some(t) = line
        .strip_prefix("kill @e[tag=")
        .and_then(|r| r.strip_suffix(']'))
        .filter(|t| !t.contains(',') && !t.contains('['))
    {
        return Some(t.to_string());
    }
    let rest = line.strip_prefix("execute as @e[tag=")?;
    let (t, tail) = rest.split_once("] run data merge entity @s {Tags:[\"")?;
    (tail.starts_with(UNSEEN_TAG) && !t.contains(',')).then(|| t.to_string())
}

/// The row a placing line states, at `fired`, with the prefix the line runs
/// under. `None` for a line that places nothing.
fn row_of_line(fired: &Firing, line: &str) -> Option<(String, Placed)> {
    if let Some(tag) = removal_tag(line) {
        return Some((String::new(), Placed::removed(fired.clone(), &tag, line)));
    }
    let (prefix, cmd) = split_run(line);
    if !prefix.is_empty()
        && cmd.starts_with("kill ")
        && let Some(tag) = removal_tag(cmd)
    {
        return Some((
            prefix.to_string(),
            Placed::removed(fired.clone(), &tag, cmd),
        ));
    }
    let row = Placed::of_summon(fired.clone(), cmd)
        .or_else(|| Placed::of_block_line(fired.clone(), cmd))?;
    Some((prefix.to_string(), row))
}

/// Which functions the ledger reads, and how each one fires: `setup_finish`
/// at setup; `activate_<obj>` at the step its objective activates on; a
/// `complete_<obj>` at its objective's step and a `complete_q_<quest>` at its
/// last objective's; a `trig_<trigger>` at the trigger's own step; a `trap_`
/// or `loop_` function unforced; and a beat's function —
/// `spawn_npc_<n>`, `despawn_npc_<n>`, `spawn_actor_<a>`, `despawn_actor_<a>`,
/// `asm_summon_<s>`, `asm_despawn_<s>` — at every step an effect that runs it
/// fires, unforced where the effect is.
fn firings_of(plan: &crate::compiler::plan::Plan, name: &str) -> Option<Vec<Firing>> {
    use crate::compiler::plan::safe_local;
    if name == "setup_finish" {
        return Some(vec![Firing::Setup]);
    }
    let objectives = || {
        plan.campaign
            .quests
            .content
            .quests
            .iter()
            .flat_map(|q| q.objectives.iter().map(move |o| (q, o)))
    };
    if let Some(safe) = name.strip_prefix("activate_o_") {
        let (_, o) = objectives().find(|(_, o)| safe_local(o.id().as_str()) == safe)?;
        return Some(vec![activation_firing(plan, o.id().as_str())]);
    }
    if let Some(safe) = name.strip_prefix("complete_o_") {
        let (q, o) = objectives().find(|(_, o)| safe_local(o.id().as_str()) == safe)?;
        let optional = plan.campaign.quest_plan.content.optional();
        return Some(vec![match plan.objective_steps.get(o.id().as_str()) {
            Some(s) if !optional.contains(q.id.as_str()) => Firing::Step(*s),
            _ => Firing::Unforced(format!(
                "the completion of `{}`, which the critical path does not force",
                o.id().as_str()
            )),
        }]);
    }
    if let Some(safe) = name.strip_prefix("complete_q_") {
        // A quest's completion fires at the step its last objective completes
        // on, when every objective of it is on the critical path.
        let q = plan
            .campaign
            .quests
            .content
            .quests
            .iter()
            .find(|q| safe_local(q.id.as_str()) == safe)?;
        let optional = plan.campaign.quest_plan.content.optional();
        let steps: Option<Vec<usize>> = q
            .objectives
            .iter()
            .map(|o| plan.objective_steps.get(o.id().as_str()).copied())
            .collect();
        return Some(vec![match steps.and_then(|s| s.into_iter().max()) {
            Some(s) if !optional.contains(q.id.as_str()) => Firing::Step(s),
            _ => Firing::Unforced(format!(
                "the completion of `{}`, which the critical path does not force",
                q.id.as_str()
            )),
        }]);
    }
    if let Some(safe) = name.strip_prefix("trig_") {
        // An environment trigger's effects fire at the step the trigger is,
        // when the critical path carries it; a trigger no step is fires when a
        // player happens to.
        let t = plan
            .campaign
            .quests
            .content
            .triggers
            .iter()
            .find(|t| safe_local(t.id.as_str()) == safe)?;
        let step = plan
            .critical_path
            .iter()
            .position(|st| crate::compiler::view::beat::step_id(st) == Some(t.id.as_str()));
        return Some(vec![match step {
            Some(s) => Firing::Step(s),
            None => Firing::Unforced(format!(
                "trigger `{}`, which the critical path does not carry",
                t.id.as_str()
            )),
        }]);
    }
    if name.starts_with("trap_") {
        return Some(vec![Firing::Unforced(
            "a trap's state or payload, which fires when a player springs it".to_string(),
        )]);
    }
    if name.starts_with("loop_") {
        return Some(vec![Firing::Unforced(
            "a loop's crossing, which fires as the party crosses it".to_string(),
        )]);
    }
    let beats = [
        ("spawn_npc_", "spawn-npc"),
        ("despawn_npc_", "despawn-npc"),
        ("spawn_actor_", "spawn-actor"),
        ("despawn_actor_", "despawn-actor"),
        ("asm_summon_", "spawn-assembly"),
        ("asm_despawn_", "despawn-assembly"),
    ];
    let (prefix, verb) = beats.iter().find(|(p, _)| name.starts_with(p))?;
    let safe = &name[prefix.len()..];
    let optional = plan.campaign.quest_plan.content.optional();
    let mut out: Vec<Firing> = Vec::new();
    crate::compiler::plan::for_each_gate_effect(plan.campaign, &mut |site, e| {
        use delvewright_dsl::Verb;
        let target = match (&e.verb, *verb) {
            (Verb::SpawnNpc { npc, .. }, "spawn-npc") => npc.as_str(),
            (Verb::DespawnNpc { npc, .. }, "despawn-npc") => npc.as_str(),
            (Verb::SpawnActor { actor }, "spawn-actor") => actor.as_str(),
            (Verb::DespawnActor { actor, .. }, "despawn-actor") => actor.as_str(),
            (Verb::SpawnAssembly { assembly }, "spawn-assembly") => assembly.as_str(),
            (Verb::DespawnAssembly { assembly }, "despawn-assembly") => assembly.as_str(),
            _ => return,
        };
        if safe_local(target) != safe {
            return;
        }
        let (step, forced) = crate::compiler::plan::firing_of(site, &plan.path_firing, &optional);
        let f = if forced {
            Firing::Step(step)
        } else {
            Firing::Unforced(format!("the `{verb}` at `{}`", site.path))
        };
        if !out.contains(&f) {
            out.push(f);
        }
    });
    if out.is_empty() {
        out.push(Firing::Unforced(format!(
            "a `{verb}` no quest effect fires (a dialogue option's)"
        )));
    }
    Some(out)
}

/// The step an objective's activation fires at: the latest critical step its
/// own step causally follows, or setup when it follows none (its quest is
/// active from the start).
pub fn activation_firing(plan: &crate::compiler::plan::Plan, objective: &str) -> Firing {
    let Some(s) = plan.objective_steps.get(objective) else {
        return Firing::Unforced(format!(
            "the activation of `{objective}`, which the critical path never reaches"
        ));
    };
    match plan
        .strict_ancestor_steps
        .get(s)
        .and_then(|a| a.iter().copied().filter(|a| a < s).max())
    {
        Some(a) => Firing::Step(a),
        None => Firing::Setup,
    }
}

/// **The ledger of a build's functions** (spec-0112 §4): every placing line of
/// the functions [`firings_of`] reads becomes a row at each firing of its
/// function, and the line is written back from the row ([`Placed::command`]),
/// so a line and its row cannot disagree. Returns the ledger and how many
/// placing lines it read.
pub fn ledger_of_functions(
    plan: &crate::compiler::plan::Plan,
    functions: &mut [(String, String)],
) -> (Ledger, usize) {
    let mut ledger = Ledger::default();
    let mut lines_read = 0usize;
    for (name, body) in functions.iter_mut() {
        let Some(firings) = firings_of(plan, name) else {
            continue;
        };
        let mut out: Vec<String> = Vec::new();
        for line in body.lines() {
            match row_of_line(&firings[0], line) {
                Some((prefix, row)) => {
                    lines_read += 1;
                    out.push(format!("{prefix}{}", row.command()));
                    for f in &firings {
                        let mut r = row.clone();
                        r.fired = f.clone();
                        r.function = name.clone();
                        ledger.rows.push(r);
                    }
                }
                None => out.push(line.to_string()),
            }
        }
        let mut rewritten = out.join("\n");
        if body.ends_with('\n') {
            rewritten.push('\n');
        }
        *body = rewritten;
    }
    (ledger, lines_read)
}

// ------------------------------------------------------------------- SNBT

/// Parse an SNBT value — the text a command's compound carries — into a tag.
/// Compounds keep their written key order. `Err` names where the text stops
/// being SNBT.
pub fn parse_snbt(text: &str) -> Result<Nbt, String> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.i != p.s.len() {
        return Err(format!("trailing text at byte {}", p.i));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected `{}` at byte {}", c as char, self.i))
        }
    }

    fn value(&mut self) -> Result<Nbt, String> {
        self.ws();
        match self.peek() {
            Some(b'{') => self.compound(),
            Some(b'[') => self.list(),
            Some(b'"') | Some(b'\'') => Ok(Nbt::Str(self.quoted()?)),
            Some(_) => self.scalar(),
            None => Err("unexpected end".to_string()),
        }
    }

    fn compound(&mut self) -> Result<Nbt, String> {
        self.expect(b'{')?;
        let mut entries = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Nbt::Compound(entries));
        }
        loop {
            self.ws();
            let key = match self.peek() {
                Some(b'"') | Some(b'\'') => self.quoted()?,
                _ => self.bare_key(),
            };
            if key.is_empty() {
                return Err(format!("a key at byte {}", self.i));
            }
            self.expect(b':')?;
            let v = self.value()?;
            entries.push((key, v));
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Nbt::Compound(entries));
                }
                _ => return Err(format!("`,` or `}}` at byte {}", self.i)),
            }
        }
    }

    fn list(&mut self) -> Result<Nbt, String> {
        self.expect(b'[')?;
        self.ws();
        // A typed array: `[B;…]`, `[I;…]`, `[L;…]`.
        if let (Some(t), Some(b';')) = (self.peek(), self.s.get(self.i + 1).copied())
            && matches!(t, b'B' | b'I' | b'L')
        {
            self.i += 2;
            let mut items = Vec::new();
            loop {
                self.ws();
                if self.peek() == Some(b']') {
                    self.i += 1;
                    break;
                }
                items.push(self.scalar()?);
                self.ws();
                if self.peek() == Some(b',') {
                    self.i += 1;
                }
            }
            let int = |n: &Nbt| match n {
                Nbt::Byte(v) => i64::from(*v),
                Nbt::Short(v) => i64::from(*v),
                Nbt::Int(v) => i64::from(*v),
                Nbt::Long(v) => *v,
                _ => 0,
            };
            return Ok(match t {
                b'B' => Nbt::ByteArray(items.iter().map(|n| int(n) as i8).collect()),
                b'I' => Nbt::IntArray(items.iter().map(|n| int(n) as i32).collect()),
                _ => Nbt::LongArray(items.iter().map(int).collect()),
            });
        }
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Nbt::List(items));
        }
        loop {
            items.push(self.value()?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Nbt::List(items));
                }
                _ => return Err(format!("`,` or `]` at byte {}", self.i)),
            }
        }
    }

    fn quoted(&mut self) -> Result<String, String> {
        let q = self.s[self.i];
        self.i += 1;
        let mut out = Vec::new();
        while let Some(c) = self.peek() {
            self.i += 1;
            if c == b'\\' {
                let Some(n) = self.peek() else {
                    return Err("an escape at the end".to_string());
                };
                self.i += 1;
                match n {
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    other => out.push(other),
                }
            } else if c == q {
                return String::from_utf8(out).map_err(|e| e.to_string());
            } else {
                out.push(c);
            }
        }
        Err("an unterminated string".to_string())
    }

    /// An unquoted compound key: the bare characters but `:`, which ends it.
    fn bare_key(&mut self) -> String {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b'+') {
                self.i += 1;
            } else {
                break;
            }
        }
        String::from_utf8_lossy(&self.s[start..self.i]).to_string()
    }

    fn bare(&mut self) -> String {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b'+' | b':') {
                self.i += 1;
            } else {
                break;
            }
        }
        String::from_utf8_lossy(&self.s[start..self.i]).to_string()
    }

    fn scalar(&mut self) -> Result<Nbt, String> {
        self.ws();
        let word = self.bare();
        if word.is_empty() {
            return Err(format!("a value at byte {}", self.i));
        }
        match word.as_str() {
            "true" => return Ok(Nbt::Byte(1)),
            "false" => return Ok(Nbt::Byte(0)),
            _ => {}
        }
        let (num, suffix) = match word.as_bytes().last() {
            Some(c) if c.is_ascii_alphabetic() => (&word[..word.len() - 1], Some(*c)),
            _ => (word.as_str(), None),
        };
        let parsed = match suffix.map(|c| c.to_ascii_lowercase()) {
            Some(b'b') => num.parse::<i8>().ok().map(Nbt::Byte),
            Some(b's') => num.parse::<i16>().ok().map(Nbt::Short),
            Some(b'l') => num.parse::<i64>().ok().map(Nbt::Long),
            Some(b'f') => num.parse::<f32>().ok().map(Nbt::Float),
            Some(b'd') => num.parse::<f64>().ok().map(Nbt::Double),
            None if !num.contains('.') && !num.contains('e') => {
                num.parse::<i32>().ok().map(Nbt::Int)
            }
            None => num.parse::<f64>().ok().map(Nbt::Double),
            _ => None,
        };
        // A bare word that is no number is a string, as the game reads one.
        Ok(parsed.unwrap_or(Nbt::Str(word)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snbt_reads_what_a_summon_carries() {
        let v = parse_snbt(
            r#"{profile:{texture:"delvewright:npc/a",model:"wide"},immovable:1b,pose:"standing",Rotation:[90f,0f],description:{text:"Keeper"},Tags:["dw_npc","dw_npc_keeper"],Pos:[1.5d,2.0d,3.5d],arr:[I;1,2,3],x:-4,y:1.25,z:'it\'s'}"#,
        )
        .unwrap();
        assert_eq!(v.get("immovable"), Some(&Nbt::Byte(1)));
        assert_eq!(
            v.get("Rotation"),
            Some(&Nbt::List(vec![Nbt::Float(90.0), Nbt::Float(0.0)]))
        );
        assert_eq!(v.get("arr"), Some(&Nbt::IntArray(vec![1, 2, 3])));
        assert_eq!(v.get("x"), Some(&Nbt::Int(-4)));
        assert_eq!(v.get("y"), Some(&Nbt::Double(1.25)));
        assert_eq!(v.get("z"), Some(&Nbt::Str("it's".into())));
        assert_eq!(
            v.get("description").and_then(|d| d.get("text")),
            Some(&Nbt::Str("Keeper".into()))
        );
    }

    #[test]
    fn a_row_writes_the_line_it_was_read_from() {
        for line in [
            "summon minecraft:interaction 5.5 65 4.5 {width:1.0f,height:2.0f,response:1b,Tags:[\"dw_borne\",\"dw_npc_keeper\"]}",
            "setblock 3 64 9 minecraft:lever[face=wall,facing=north]",
            "item replace block 1 65 2 container.0 with minecraft:emerald 3",
        ] {
            let row = Placed::of_summon(Firing::Setup, line)
                .or_else(|| Placed::of_block_line(Firing::Setup, line))
                .unwrap();
            assert_eq!(row.command(), line);
        }
        let row = Placed::of_summon(
            Firing::Setup,
            "summon minecraft:mannequin 5.5 65 4.5 {Rotation:[180f,0f],Tags:[\"dw_npc\",\"dw_npc_k\"]}",
        )
        .unwrap();
        let e = row.entity_record().unwrap();
        assert_eq!(e.pos, [5.5, 65.0, 4.5]);
        assert_eq!(e.yaw, 180.0);
        assert_eq!(e.tags, ["dw_npc", "dw_npc_k"]);
    }
}
