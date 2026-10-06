//! **A teleport the route proof can take** (spec-0083).
//!
//! The route model has two ways the party moves: a walk, and the crossing the
//! compiler itself authors when consecutive objectives stand in different areas
//! ([`crate::compiler::plan::Plan::transport`]). An authored `teleport` is a third
//! kind of carry, and this module decides which authored teleports the proof may
//! lean on and which it may not.
//!
//! ## Links and gathers
//!
//! A **link** is a `teleport` hosted in a `triggers[]` entry declared
//! `once: false` — directly in its bundle or in one of its `sequence` steps —
//! whose `from` and `to` anchors stand in one area. It is the one carry a
//! straggler can use: whoever is left on the jetty presses the tiller again and
//! follows. Every other teleport is a **gather**: whoever is in the box travels,
//! once, and the route proof never leans on it.
//!
//! The property that makes the carry re-usable is `once: false`, and it belongs
//! to the trigger, which is the object that fires — so the rule sits at the root
//! and is not a flag on the verb.
//!
//! ## One record
//!
//! [`collect`] is the single enumeration. The plan carries its result
//! ([`crate::compiler::plan::Plan::links`] /
//! [`crate::compiler::plan::Plan::gathers`]); the route proof, the branch proof,
//! the waypoint export, the bot contract, the leave proof and the two party
//! populations all read the trigger step the path performs from one link
//! (`Step::Trigger::stand` plus its `transport` marker), so a link cannot be
//! seen by one proof and not another.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{
    Campaign, CompareOp, Diagnostic, QuestEffect, StateCompare, TriggerOn, Verb,
};

use crate::compiler::plan::{
    AnchorTable, DW_TELEPORT_CARRY_UNREALISED, DW_TELEPORT_UNDER_CUTSCENE, EffectRoot,
    ResolvedAnchor, for_each_effect_root,
};

/// One region write a link's own root performs, at its tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootWrite {
    /// The `sequence` tick it fires at (`0` in a flat bundle).
    pub tick: u32,
    /// The written box's inclusive corners.
    pub region: ([i32; 3], [i32; 3]),
    /// `true` for a fill of a full block, `false` for a clear. A fluid fill is
    /// neither floor nor way and is recorded as a fill that lays no footing.
    pub fill: bool,
    /// `true` when the fill is a fluid — impassable and never floor.
    pub fluid: bool,
}

/// A **link**: a repeatable trigger that carries whoever stands in its volume
/// onto a cell of the same area (spec-0083 §3.1).
#[derive(Clone, Debug)]
pub struct LinkPlan {
    /// The hosting `trigger/<id>`.
    pub trigger_id: String,
    /// The trigger's event as its kebab tag.
    pub on: &'static str,
    /// The watched anchor (absent for `strike-npc`).
    pub anchor_id: Option<String>,
    /// The watched NPC (`strike-npc` only).
    pub npc_id: Option<String>,
    /// The watched assembly (`strike-assembly` only, spec-0082).
    pub assembly_id: Option<String>,
    /// An `approach` trigger's radius.
    pub range: Option<u32>,
    /// The cells of the trigger's body: the anchor's point, or every cell of a
    /// gate region it rides. Empty for `strike-npc`, whose body is the NPC's at
    /// the beat the path performs it.
    pub body: Vec<[i32; 3]>,
    /// The JSON pointer of the `teleport` effect.
    pub path: String,
    /// The `from` anchor's name.
    pub from_anchor: String,
    /// The `from` volume's inclusive corners.
    pub from: ([i32; 3], [i32; 3]),
    /// The area the `from` anchor resolves in.
    pub from_area: String,
    /// The `to` mark's anchor name.
    pub to_anchor: String,
    /// The area the `to` anchor resolves in.
    pub to_area: String,
    /// The cell the teleport puts a body on.
    pub to: [i32; 3],
    /// The `sequence` tick the teleport fires at (`0` in a flat bundle).
    pub tick: u32,
    /// The trigger's positive flag gate.
    pub requires_flags: Vec<String>,
    /// The trigger's negative flag gate.
    pub forbids_flags: Vec<String>,
    /// Every numeric term that must hold — the trigger's and every `when` on
    /// the way down to the teleport.
    pub requires_state: Vec<StateCompare>,
    /// The `when` flag terms on the way down to the teleport (the enclosing
    /// `sequence`'s and the teleport's own).
    pub when_requires: Vec<String>,
    /// The `when` negative flag terms on the way down to the teleport.
    pub when_forbids: Vec<String>,
    /// Every `fill-region`/`clear-region` the same root performs, with its tick.
    pub writes: Vec<RootWrite>,
}

impl LinkPlan {
    /// Whether `cell` lies inside the `from` volume.
    pub fn contains(&self, cell: [i32; 3]) -> bool {
        let (lo, hi) = self.from;
        (0..3).all(|i| lo[i] <= cell[i] && cell[i] <= hi[i])
    }

    /// Whether the flag half of this link's gate is open under `held`.
    pub fn flags_open(&self, held: &BTreeSet<String>) -> bool {
        self.requires_flags
            .iter()
            .chain(&self.when_requires)
            .all(|f| held.contains(f))
            && !self
                .forbids_flags
                .iter()
                .chain(&self.when_forbids)
                .any(|f| held.contains(f))
    }

    /// Whether the numeric half of this link's gate is open under `data` —
    /// every term compares true against a value the path's writes establish.
    /// A datum no ordered walk can name opens nothing.
    pub fn state_open(&self, data: &BTreeMap<String, Option<i64>>) -> bool {
        self.requires_state.iter().all(|t| {
            data.get(t.state.as_str())
                .copied()
                .flatten()
                .is_some_and(|v| compare(v, t.op, i64::from(t.value)))
        })
    }

    /// The box in words, for a message.
    pub fn box_words(&self) -> String {
        let (lo, hi) = self.from;
        format!(
            "[{}, {}, {}]..[{}, {}, {}]",
            lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
        )
    }
}

/// A **gather**: every `teleport` that is not a link. Whoever is in the box
/// travels, once; the route proof never leans on it.
#[derive(Clone, Debug)]
pub struct GatherPlan {
    /// The JSON pointer of the `teleport` effect.
    pub path: String,
    /// The root it fires from, in words.
    pub root: String,
    /// The `from` volume's inclusive corners.
    pub from: ([i32; 3], [i32; 3]),
    /// The cell it puts a body on.
    pub to: [i32; 3],
}

impl GatherPlan {
    /// Whether `cell` lies inside the `from` volume.
    pub fn contains(&self, cell: [i32; 3]) -> bool {
        let (lo, hi) = self.from;
        (0..3).all(|i| lo[i] <= cell[i] && cell[i] <= hi[i])
    }
}

fn compare(v: i64, op: CompareOp, k: i64) -> bool {
    match op {
        CompareOp::Equals => v == k,
        CompareOp::NotEquals => v != k,
        CompareOp::AtLeast => v >= k,
        CompareOp::AtMost => v <= k,
    }
}

/// The first `(area, resolved)` an anchor name resolves to, in key order — the
/// same reading [`crate::compiler::plan::Plan::zone_box`] makes.
fn resolve<'t>(anchors: &'t AnchorTable, name: &str) -> Option<(&'t str, &'t ResolvedAnchor)> {
    anchors
        .iter()
        .find(|((_, n), _)| n == name)
        .map(|((a, _), r)| (a.as_str(), r))
}

fn point_of(r: &ResolvedAnchor) -> [i32; 3] {
    match r {
        ResolvedAnchor::Point { pos, .. } => *pos,
        ResolvedAnchor::Gate { from, .. } => *from,
    }
}

fn cells_of(r: &ResolvedAnchor) -> Vec<[i32; 3]> {
    match r {
        ResolvedAnchor::Point { pos, .. } => vec![*pos],
        ResolvedAnchor::Gate { from, to, .. } => {
            crate::compiler::assembled::region_cells(*from, *to).collect()
        }
    }
}

fn zone(anchors: &AnchorTable, z: &delvewright_dsl::StealthZone) -> Option<([i32; 3], [i32; 3])> {
    let (_, r) = resolve(anchors, z.anchor.as_str())?;
    let c = point_of(r);
    let e = z.extent;
    Some((
        [c[0] - e[0] as i32, c[1] - e[1] as i32, c[2] - e[2] as i32],
        [c[0] + e[0] as i32, c[1] + e[1] as i32, c[2] + e[2] as i32],
    ))
}

/// One effect of a root's timeline: the effect, its tick, the guards on the
/// way down to it, and its JSON pointer.
struct Timed<'a> {
    eff: &'a QuestEffect,
    tick: u32,
    guards: Vec<&'a delvewright_dsl::Guard>,
    path: String,
}

/// A root list's own timeline: its flat effects at tick `0`, and every effect
/// of a `sequence` step at that step's tick. A nested list that fires at
/// another moment (`on_respawn`, `on_rest`, `on_caught`, `on_arrive`) is its own
/// timeline, returned beside this one so nothing is lost and nothing is
/// misdated.
fn timelines<'a>(list: &'a [QuestEffect], path: &str) -> Vec<Vec<Timed<'a>>> {
    let mut out: Vec<Vec<Timed<'a>>> = vec![Vec::new()];
    for (i, eff) in list.iter().enumerate() {
        let p = format!("{path}/{i}");
        let mut guards = Vec::new();
        if let Some(g) = &eff.when {
            guards.push(g);
        }
        out[0].push(Timed {
            eff,
            tick: 0,
            guards: guards.clone(),
            path: p.clone(),
        });
        match &eff.verb {
            Verb::Sequence { steps } => {
                for (s, st) in steps.iter().enumerate() {
                    for (j, inner) in st.effects.iter().enumerate() {
                        let ip = format!("{p}/steps/{s}/effects/{j}");
                        let mut g2 = guards.clone();
                        if let Some(g) = &inner.when {
                            g2.push(g);
                        }
                        out[0].push(Timed {
                            eff: inner,
                            tick: st.at_ticks,
                            guards: g2,
                            path: ip.clone(),
                        });
                        for (pseg, _, nested) in inner.nested_effect_lists_labeled() {
                            out.extend(timelines(nested, &format!("{ip}/{pseg}")));
                        }
                    }
                }
            }
            _ => {
                for (pseg, _, nested) in eff.nested_effect_lists_labeled() {
                    out.extend(timelines(nested, &format!("{p}/{pseg}")));
                }
            }
        }
    }
    out
}

/// The root a gather fires from, in words.
fn root_words(root: &EffectRoot<'_>) -> String {
    match root {
        EffectRoot::ObjectiveComplete { objective, .. } => {
            format!("the completion of `{objective}`")
        }
        EffectRoot::QuestComplete(q) => format!("the completion of `{}`", q.id.as_str()),
        EffectRoot::Trigger(t) if t.once => {
            format!("`{}`, a trigger that fires once", t.id.as_str())
        }
        EffectRoot::Trigger(t) => format!("`{}`", t.id.as_str()),
        EffectRoot::TrapPayload(t) => format!("trap `{}`'s payload", t.id.as_str()),
        EffectRoot::DialogueRespawn => "a respawn bundle".to_string(),
        EffectRoot::ShortcutUnlock => "a shortcut's far side".to_string(),
        EffectRoot::OnDeath => "`on_death`".to_string(),
        EffectRoot::ShopOffer => "a shop offer".to_string(),
        EffectRoot::OnKill(_) => "an `on_kill` bundle".to_string(),
        EffectRoot::AssemblyLand(m) => format!("assembly `{}`'s strike landing", m.id.as_str()),
        EffectRoot::LoopCross(l) => format!("loop `{}`'s `on_cross`", l.id.as_str()),
    }
}

/// Every authored teleport's source volume — each link's `from`, then each
/// gather's, in [`collect`] order. A teleport whose anchors do not resolve is in
/// neither list (`DW0360` owns that failure). Read by spec-0086's slab and span
/// checks, where a body in a teleport volume would be carried twice.
pub fn source_volumes(plan: &crate::compiler::plan::Plan<'_>) -> Vec<([i32; 3], [i32; 3])> {
    plan.links
        .iter()
        .map(|l| l.from)
        .chain(plan.gathers.iter().map(|g| g.from))
        .collect()
}

/// **The one enumeration of authored teleports**, split into links and
/// gathers, in [`for_each_effect_root`] order (ADR-0006). A teleport whose
/// anchors do not resolve is in neither list: `DW0360` owns that failure.
pub fn collect(campaign: &Campaign, anchors: &AnchorTable) -> (Vec<LinkPlan>, Vec<GatherPlan>) {
    let mut links = Vec::new();
    let mut gathers = Vec::new();
    for_each_effect_root(campaign, &mut |site, list| {
        let lines = timelines(list, &site.path);
        for (k, line) in lines.iter().enumerate() {
            let writes: Vec<RootWrite> = line
                .iter()
                .filter_map(|t| {
                    let (z, block) = t.eff.region_write()?;
                    let region = zone(anchors, z)?;
                    let fluid = block.is_some_and(crate::compiler::assembled::is_fluid);
                    Some(RootWrite {
                        tick: t.tick,
                        region,
                        fill: block.is_some(),
                        fluid,
                    })
                })
                .collect();
            for t in line {
                let Some((from, to)) = t.eff.teleport() else {
                    continue;
                };
                let (Some(from_box), Some((from_area, _)), Some((to_area, to_r))) = (
                    zone(anchors, from),
                    resolve(anchors, from.anchor.as_str()),
                    resolve(anchors, to.anchor.as_str()),
                ) else {
                    continue;
                };
                let to_cell = to.cell(point_of(to_r));
                // The press's own timeline only: a nested bundle (`on_rest`,
                // `on_arrive`, …) fires at another moment and its carry is a
                // gather.
                let host = match site.root {
                    EffectRoot::Trigger(trig) if !trig.once && k == 0 => Some(trig),
                    _ => None,
                };
                let Some(trig) = host else {
                    gathers.push(GatherPlan {
                        path: t.path.clone(),
                        root: root_words(&site.root),
                        from: from_box,
                        to: to_cell,
                    });
                    continue;
                };
                let body = match trig.at_anchor().and_then(|a| resolve(anchors, a)) {
                    Some((_, r)) => cells_of(r),
                    None => Vec::new(),
                };
                let mut requires_state = trig.requires_state.clone();
                let mut when_requires = Vec::new();
                let mut when_forbids = Vec::new();
                for g in &t.guards {
                    when_requires.extend(g.requires_flags.iter().map(|f| f.as_str().to_string()));
                    when_forbids.extend(g.forbids_flags.iter().map(|f| f.as_str().to_string()));
                    requires_state.extend(g.requires_state.iter().cloned());
                }
                links.push(LinkPlan {
                    trigger_id: trig.id.as_str().to_string(),
                    on: trig.on.kind(),
                    anchor_id: trig.at_anchor().map(str::to_string),
                    npc_id: trig.on.npc_target().map(|n| n.as_str().to_string()),
                    assembly_id: trig.on.assembly_target().map(|a| a.as_str().to_string()),
                    range: match trig.on {
                        TriggerOn::Approach { range } => Some(range),
                        _ => None,
                    },
                    body,
                    path: t.path.clone(),
                    from_anchor: from.anchor.as_str().to_string(),
                    from: from_box,
                    from_area: from_area.to_string(),
                    to_anchor: to.anchor.as_str().to_string(),
                    to_area: to_area.to_string(),
                    to: to_cell,
                    tick: t.tick,
                    requires_flags: trig
                        .requires_flags
                        .iter()
                        .map(|f| f.as_str().to_string())
                        .collect(),
                    forbids_flags: trig
                        .forbids_flags
                        .iter()
                        .map(|f| f.as_str().to_string())
                        .collect(),
                    requires_state,
                    when_requires,
                    when_forbids,
                    writes: writes.clone(),
                });
            }
        }
    });
    (links, gathers)
}

/// The static half of `DW0932` (spec-0083 §3.6): the two faults the plan can
/// judge before a block is read — `to` inside its own `from`, and `from` and
/// `to` in two areas. The geometric half (no stand cell, `to` not standable at
/// the teleport's tick) needs the world and is judged where the proof takes the
/// link.
pub fn static_fault(l: &LinkPlan) -> Option<String> {
    if l.from_area != l.to_area {
        return Some(format!(
            "link `{}` (the `teleport` at `{}`) carries a body from `{}` in area `{}` to `{}` in \
             area `{}`. A link joins two places of ONE area; a move between areas is the \
             compiler's own crossing, which has its own arrival and its own refusals. Fault: \
             `from` and `to` in different areas. Remedy: use a crossing for another area — put \
             the next objective in that area and let the compiler carry the party there.",
            l.trigger_id, l.path, l.from_anchor, l.from_area, l.to_anchor, l.to_area
        ));
    }
    if l.contains(l.to) {
        return Some(format!(
            "link `{}` (the `teleport` at `{}`) puts a body on {:?}, which lies inside its own \
             volume {} — a teleport onto a cell inside its own box is a regroup, not a way \
             onward. Fault: `to` inside `from`. Remedy: move `to` off the volume and onto \
             footing in the place the link leads to.",
            l.trigger_id,
            l.path,
            l.to,
            l.box_words()
        ));
    }
    None
}

/// The tick a cutscene's `cs_end` runs at, counted from the tick the cutscene
/// starts: one past the driver's last frame. The same sum
/// `emit::cutscene_fns` lays its shots end to end by — `shot_ticks` of each
/// shot's resolved seconds, plus one tick of hard cut each.
pub fn cutscene_end_offset(eff: &QuestEffect) -> Option<u32> {
    let shots = eff.cutscene_shots().filter(|s| !s.is_empty())?;
    let total: i32 = shots
        .iter()
        .map(|s| crate::compiler::camera::shot_ticks(s.resolved_seconds()) + 1)
        .sum();
    Some(total.max(0) as u32)
}

/// `DW0933` (spec-0083 §3.5): a `teleport` — link or gather — that fires at or
/// before the tick its root's `cutscene` ends. `cs_end` puts every player back
/// on the marker saved at the cutscene's start, so a teleport under an open
/// bracket is undone for everyone the moment the camera returns.
pub fn check_teleport_under_cutscene(campaign: &Campaign) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    for_each_effect_root(campaign, &mut |site, list| {
        for line in timelines(list, &site.path) {
            let scenes: Vec<(&Timed<'_>, u32)> = line
                .iter()
                .filter_map(|t| cutscene_end_offset(t.eff).map(|off| (t, t.tick + off)))
                .collect();
            if scenes.is_empty() {
                continue;
            }
            for t in &line {
                if t.eff.teleport().is_none() {
                    continue;
                }
                let Some((scene, end)) = scenes
                    .iter()
                    .find(|(s, end)| s.tick <= t.tick && t.tick <= *end)
                else {
                    continue;
                };
                let flat = t.tick == 0 && scene.tick == 0 && !t.path.contains("/steps/");
                let shape = if flat {
                    "in one flat bundle with the cutscene".to_string()
                } else {
                    format!("at tick {}", t.tick)
                };
                d.push(Diagnostic::error(
                    DW_TELEPORT_UNDER_CUTSCENE,
                    site.stage,
                    &t.path,
                    format!(
                        "the `teleport` at `{tp}` fires {shape}, while the cutscene at `{cs}` \
                         is still playing — its `cs_end` runs at tick {end} of this root, and \
                         `cs_end` puts every player back on the marker it saved when the \
                         cutscene started, so the carry is undone for everyone when the camera \
                         returns. Remedy: put the teleport in a `sequence` step at tick {first} \
                         or later.",
                        tp = t.path,
                        cs = scene.path,
                        first = end + 1,
                    ),
                ));
            }
        }
    });
    d
}

/// The node of the layout graph an anchor belongs to: a station a node
/// declares, or the node a synthesized `anchor/node-<slug>` names.
fn node_of(graph: &delvewright_dsl::layout::LayoutGraphContent, anchor: &str) -> Option<String> {
    for n in &graph.nodes {
        if n.stations.iter().any(|s| s.anchor.as_str() == anchor) {
            return Some(n.id.0.clone());
        }
        let slug = n.id.0.strip_prefix("node/").unwrap_or(&n.id.0);
        if anchor == format!("anchor/node-{slug}") {
            return Some(n.id.0.clone());
        }
    }
    None
}

/// A link as the graph sees it, read off the documents alone: the hosting
/// trigger, the teleport's pointer, and the two anchors.
struct DocLink<'a> {
    trigger: &'a str,
    path: String,
    from: &'a str,
    to: &'a str,
}

fn doc_links(campaign: &Campaign) -> Vec<DocLink<'_>> {
    let mut out = Vec::new();
    for_each_effect_root(campaign, &mut |site, list| {
        let EffectRoot::Trigger(trig) = site.root else {
            return;
        };
        if trig.once {
            return;
        }
        let lines = timelines(list, &site.path);
        for t in lines.first().into_iter().flatten() {
            if let Some((from, to)) = t.eff.teleport() {
                out.push(DocLink {
                    trigger: trig.id.as_str(),
                    path: t.path.clone(),
                    from: from.anchor.as_str(),
                    to: to.anchor.as_str(),
                });
            }
        }
    });
    out
}

/// `DW0934` (spec-0083 §7): the layout graph and the quests disagree about
/// carries — a `carry` direction no link realises, or a link whose `from`
/// station and `to` mark stand in two places no `carry` edge joins in that
/// direction. Runs only for a campaign that carries a layout graph.
///
/// A link whose two anchors stand in ONE place is a move inside that place and
/// owes the graph nothing. A link whose anchor belongs to no place is refused,
/// because the graph then cannot say what it joins.
pub fn check_carry_realised(campaign: &Campaign) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let Some(graph) = campaign.layout_graph.as_ref().map(|g| &g.content) else {
        return d;
    };
    let links = doc_links(campaign);
    let carries: Vec<(usize, &delvewright_dsl::layout::Edge)> = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, delvewright_dsl::layout::Edge::Carry { .. }))
        .collect();
    let mut realised: BTreeSet<(String, String)> = BTreeSet::new();
    for l in &links {
        let (a, b) = (node_of(graph, l.from), node_of(graph, l.to));
        let (Some(a), Some(b)) = (a, b) else {
            d.push(Diagnostic::error(
                DW_TELEPORT_CARRY_UNREALISED,
                "quests",
                &l.path,
                format!(
                    "link `{}` carries a body from `{}` to `{}`, and the layout graph places {} \
                     in no node — a station a node declares, or a node's own anchor. The graph \
                     cannot say which places this link joins. Remedy: declare the anchor as a \
                     station of the place it stands in, then draw the `carry` edge between the \
                     two places.",
                    l.trigger,
                    l.from,
                    l.to,
                    match (node_of(graph, l.from), node_of(graph, l.to)) {
                        (None, None) => format!("`{}` and `{}`", l.from, l.to),
                        (None, _) => format!("`{}`", l.from),
                        _ => format!("`{}`", l.to),
                    }
                ),
            ));
            continue;
        };
        if a == b {
            continue;
        }
        let joined = carries.iter().any(|(_, e)| {
            let (ea, eb) = (e.a().0.as_str(), e.b().0.as_str());
            let fwd = e.direction() != Some(delvewright_dsl::layout::Direction::BToA);
            let back = e.direction() != Some(delvewright_dsl::layout::Direction::AToB);
            (fwd && ea == a && eb == b) || (back && eb == a && ea == b)
        });
        if joined {
            realised.insert((a.clone(), b.clone()));
        } else {
            d.push(Diagnostic::error(
                DW_TELEPORT_CARRY_UNREALISED,
                "quests",
                &l.path,
                format!(
                    "link `{}` carries a body from `{a}` (its station `{}`) to `{b}` (its mark \
                     `{}`), and no `carry` edge of the layout graph joins `{a}` to `{b}` in that \
                     direction — the graph's claim about how its places connect is missing a \
                     way the quests use. Remedy: draw the edge — `{{\"class\": \"carry\", \"a\": \
                     \"{a}\", \"b\": \"{b}\", \"gating\": …}}` — or host the teleport on a \
                     trigger as a link only where the graph says the party is carried.",
                    l.trigger, l.from, l.to
                ),
            ));
        }
    }
    for (i, e) in carries {
        let (ea, eb) = (e.a().0.clone(), e.b().0.clone());
        let mut owed = Vec::new();
        if e.direction() != Some(delvewright_dsl::layout::Direction::BToA) {
            owed.push((ea.clone(), eb.clone()));
        }
        if e.direction() != Some(delvewright_dsl::layout::Direction::AToB) {
            owed.push((eb.clone(), ea.clone()));
        }
        for (x, y) in owed {
            if !realised.contains(&(x.clone(), y.clone())) {
                d.push(Diagnostic::error(
                    DW_TELEPORT_CARRY_UNREALISED,
                    "layout-graph",
                    format!("/content/edges/{i}"),
                    format!(
                        "carry connection `{}` says a body is carried from `{x}` to `{y}`, and no \
                         link realises that direction — no `triggers[]` entry declared \
                         `once: false` hosts a `teleport` from a station of `{x}` to a mark in \
                         `{y}`. A carry nothing performs is a way the graph claims and the delve \
                         does not have. Remedy: host a teleport from a station of `{x}` to a mark in `{y}` \
                         on a repeatable trigger, or make the edge one-way (or remove it) if \
                         that direction is not a way.",
                        e.id()
                    ),
                ));
            }
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_reads_every_operator() {
        assert!(compare(1, CompareOp::Equals, 1));
        assert!(compare(1, CompareOp::NotEquals, 2));
        assert!(compare(3, CompareOp::AtLeast, 3));
        assert!(!compare(2, CompareOp::AtLeast, 3));
        assert!(compare(2, CompareOp::AtMost, 2));
    }
}
