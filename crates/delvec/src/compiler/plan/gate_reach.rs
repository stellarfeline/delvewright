//! Gate-aware reachability (`DW0306`): the piece graph with its gates, and the
//! deadlock proof over it.

use super::*;

// ---------------------------------------------------------------------------
// Gate-aware reachability (M2 fix 7, DW0306)
// ---------------------------------------------------------------------------

/// A gate inside a placed piece: the carrying piece index and the local plane
/// (`axis`, `plane`) the barred row sits on. A sealed gate splits its piece into
/// the `local[axis] < plane` half (side 0) and the `>= plane` half (side 1); the
/// only path between them is the gate cut-edge, present once the gate is opened.
struct GateInfo {
    piece: usize,
    axis: usize,
    plane: i32,
}

/// A placed connector socket in world space, for reconstructing which pieces mate.
struct WorldSocket {
    piece: usize,
    connector: usize,
    world: [i32; 3],
    facing: Facing,
}

/// A piece-connectivity graph node: `(piece index, gate side)`. Non-gate pieces
/// are always side 0.
type Node = (usize, u8);

/// Verify every objective anchored in `area_id` is reachable from the area's
/// entry point using only gates already opened by earlier objectives in the DAG
/// order ([`DW_GATE_DEADLOCK`]). No-op for areas without gates.
///
/// `entry` is the anchor name [`AnchorTable::entry_anchor_name`] resolved for
/// this area — handed in rather than looked up, because this proof reads its
/// start node out of prefab metadata (it needs the *piece* the anchor sits in,
/// which the resolved map does not record) and a second lookup here would be a
/// second place that decides what an entry is.
pub(super) fn check_gate_reachability(
    campaign: &Campaign,
    area_id: &str,
    entry: Option<&str>,
    pieces: &[PiecePlacement],
    registry: &PrefabRegistry,
    severed: Option<&BTreeSet<[i32; 3]>>,
) -> Result<(), PlanError> {
    // Gates carried by a piece in this area.
    let mut gates: BTreeMap<String, GateInfo> = BTreeMap::new();
    for name in collect_open_gate_anchors(campaign) {
        if let Some((pi, meta)) = anchor_piece(pieces, registry, &name)
            && let Some(region) = &meta.region
            && let Some((axis, plane)) = gate_plane(region)
        {
            gates.insert(
                name,
                GateInfo {
                    piece: pi,
                    axis,
                    plane,
                },
            );
        }
    }
    if gates.is_empty() {
        return Ok(());
    }

    // Global critical objective order (quests topo-sorted, objectives by `after`).
    let order = critical_objective_order(campaign);
    // When each gate opens: the earliest order index whose objective/quest opens it.
    let gate_open_at = gate_open_indices(campaign, &order, &gates);

    // Static (gate-independent) adjacency: mated sockets between pieces.
    let sockets = world_sockets(pieces, registry);
    let adj = build_adjacency(&sockets, pieces, registry, &gates, severed);

    // The entry piece: the anchor the ONE resolver picked for this area. The
    // proof must start where the player actually starts, and what an entry is
    // is not this function's question.
    let Some(spawn) = entry.and_then(|name| anchor_node(pieces, registry, name, &gates)) else {
        return Ok(()); // no entry anchor in this area → DW0345 reports it at build
    };

    for (i, step) in order.iter().enumerate() {
        let Some((tarea, tname)) = objective_target(campaign, step.obj, step.area) else {
            continue;
        };
        if tarea != area_id {
            continue;
        }
        let Some(target) = anchor_node(pieces, registry, &tname, &gates) else {
            continue;
        };
        // Gates already open when the player must stand at this objective's anchor:
        // those an earlier objective (index < i) has opened.
        let open: BTreeSet<usize> = gates
            .values()
            .zip(gates.keys())
            .filter(|(_, name)| gate_open_at.get(*name).is_some_and(|&j| j < i))
            .map(|(g, _)| g.piece)
            .collect();
        if !reachable(spawn, target, &adj, &gates, &open) {
            let culprit = gates
                .iter()
                .filter(|(name, _)| gate_open_at.get(*name).is_none_or(|&j| j >= i))
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            // A `rewire-socket sealed` (spec-0017) cuts doorway edges out
            // of this graph, so the blockage may be a massed-away passage, not
            // a quest-order mistake — say so.
            let severed_note = if severed.is_some_and(|s| !s.is_empty()) {
                " NOTE: this area's world-edits script seals doorway socket(s) via \
                 `rewire-socket` — those passages are cut from this proof; if the blockage \
                 is one of them, reopen it or leave another route"
            } else {
                ""
            };
            return Err(PlanError::new(
                DW_GATE_DEADLOCK,
                format!(
                    "objective `{}` (anchor `{tname}` in area `{area_id}`) is only reachable \
                     through a gate that no earlier objective opens (sealed gate(s): {culprit}), \
                     so the delve deadlocks. Fix the quest order: add an earlier objective whose \
                     `open-gate` effect opens {culprit} before this objective, or move `{tname}` \
                     to the near side of the gate. Do NOT delete the gate to dodge the check — \
                     that removes intended progression.{severed_note}",
                    step.obj.id()
                ),
            ));
        }
    }
    Ok(())
}

/// One objective in critical order, with its owning quest area.
struct OrderedObj<'a> {
    obj: &'a Objective,
    quest: &'a str,
    area: &'a str,
}

/// Every objective in critical-path order — the branch-coherent playthrough
/// ([`crate::compiler::flow::Flow::playthrough`]), i.e. exactly the sequence
/// [`build_critical_path`] exports, so the gate-aware reachability proof
/// (`DW0306`) judges the same walk the bot will.
fn critical_objective_order(campaign: &Campaign) -> Vec<OrderedObj<'_>> {
    let path = crate::compiler::flow::Flow::new(campaign).playthrough();
    let stage5: BTreeMap<&str, &Quest> = campaign
        .quests
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q))
        .collect();
    let quest_area: BTreeMap<&str, &str> = campaign
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.area.as_str()))
        .collect();
    let mut out = Vec::new();
    for st in &path.steps {
        let Some(q) = stage5.get(st.quest.as_str()) else {
            continue;
        };
        let Some(obj) = q
            .objectives
            .iter()
            .find(|o| o.id().as_str() == st.objective)
        else {
            continue;
        };
        out.push(OrderedObj {
            obj,
            quest: q.id.as_str(),
            area: quest_area.get(st.quest.as_str()).copied().unwrap_or(""),
        });
    }
    out
}

/// The `(area, anchor)` a player must stand at to complete `obj`.
fn objective_target(
    campaign: &Campaign,
    obj: &Objective,
    quest_area: &str,
) -> Option<(String, String)> {
    match obj {
        Objective::TalkTo { npc, .. } => {
            let n = campaign
                .npcs
                .content
                .npcs
                .iter()
                .find(|n| n.id.as_str() == npc.as_str())?;
            Some((n.area.as_str().to_string(), n.anchor.as_str().to_string()))
        }
        Objective::ReachAnchor { anchor, .. }
        | Objective::Collect { anchor, .. }
        | Objective::Interact { anchor, .. } => {
            Some((quest_area.to_string(), anchor.as_str().to_string()))
        }
        Objective::Kill { wave, .. } => {
            let w = wave_of(campaign, wave.as_str())?;
            Some((quest_area.to_string(), w.anchor.as_str().to_string()))
        }
    }
}

/// Every anchor named by an `open-gate` effect anywhere in the campaign — quest
/// effects **and** environment triggers, descending every nested effect list
/// ([`QuestEffect::visit_deep`]). A gate opened from inside a `sequence` step or an
/// `on_arrive` bundle is a real gate: missing it here dropped the gate out of the
/// deadlock proof's model entirely, so `DW0306` silently proved reachability
/// against a world with one fewer door than the delve ships.
fn collect_open_gate_anchors(campaign: &Campaign) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut note = |e: &QuestEffect| {
        e.visit_deep(&mut |inner| {
            if let Some(a) = inner.open_gate_anchor() {
                out.insert(a.as_str().to_string());
            }
        });
    };
    for q in &campaign.quests.content.quests {
        for effs in q.on_objective_complete.values() {
            for e in effs {
                note(e);
            }
        }
        for e in &q.on_complete {
            note(e);
        }
    }
    for t in &campaign.quests.content.triggers {
        for e in &t.effects {
            note(e);
        }
    }
    out
}

/// The earliest critical-order index at which each gate opens: the index of the
/// objective whose `on_objective_complete` opens it, or the last objective of a
/// quest whose `on_complete` opens it (min over all openers).
fn gate_open_indices(
    campaign: &Campaign,
    order: &[OrderedObj<'_>],
    gates: &BTreeMap<String, GateInfo>,
) -> BTreeMap<String, usize> {
    let index_of: BTreeMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, s)| (s.obj.id().as_str(), i))
        .collect();
    let last_obj_index: BTreeMap<&str, usize> = {
        let mut m: BTreeMap<&str, usize> = BTreeMap::new();
        for (i, s) in order.iter().enumerate() {
            m.entry(s.quest)
                .and_modify(|e| *e = (*e).max(i))
                .or_insert(i);
        }
        m
    };
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    let note = |gate: &str, idx: usize, out: &mut BTreeMap<String, usize>| {
        if gates.contains_key(gate) {
            out.entry(gate.to_string())
                .and_modify(|e| *e = (*e).min(idx))
                .or_insert(idx);
        }
    };
    // Deep + trigger-aware, mirroring `collect_open_gate_anchors`: a gate opened
    // from a `sequence` step opens at the step that fires the sequence, and a
    // trigger-fired gate is conservatively treated as open from step 0 (a trigger
    // has no place in the objective DAG — the same conservative rooting
    // `collect_region_events` uses). Treating either as "never opened" is what made
    // the deadlock proof reject a perfectly playable delve.
    let deep = |e: &QuestEffect, idx: usize, out: &mut BTreeMap<String, usize>| {
        e.visit_deep(&mut |inner| {
            if let Some(a) = inner.open_gate_anchor() {
                note(a.as_str(), idx, out);
            }
        });
    };
    for q in &campaign.quests.content.quests {
        for (oid, effs) in &q.on_objective_complete {
            let Some(&idx) = index_of.get(oid.as_str()) else {
                continue;
            };
            for e in effs {
                deep(e, idx, &mut out);
            }
        }
        if let Some(&idx) = last_obj_index.get(q.id.as_str()) {
            for e in &q.on_complete {
                deep(e, idx, &mut out);
            }
        }
    }
    for t in &campaign.quests.content.triggers {
        for e in &t.effects {
            deep(e, 0, &mut out);
        }
    }
    out
}

/// The gate's local dividing plane: the horizontal axis (x or z) the barred row is
/// thin along, plus the coordinate of that plane. `None` if neither horizontal
/// axis is a single cell thick (not a wall-like gate).
fn gate_plane(region: &crate::compiler::registry::Region) -> Option<(usize, i32)> {
    let span = |a: usize| (region.from[a] - region.to[a]).abs();
    // Prefer the thinner of the two horizontal axes (a vertical gate wall).
    let (zx, xx) = (span(2), span(0));
    if zx <= xx && zx == 0 {
        Some((2, region.from[2]))
    } else if xx == 0 {
        Some((0, region.from[0]))
    } else {
        None
    }
}

/// The placed piece carrying `anchor_name`, with that anchor's local metadata.
fn anchor_piece<'a>(
    pieces: &[PiecePlacement],
    registry: &'a PrefabRegistry,
    anchor_name: &str,
) -> Option<(usize, &'a AnchorMeta)> {
    pieces.iter().enumerate().find_map(|(i, p)| {
        registry
            .get(&p.prefab_id)
            .and_then(|m| m.anchors.get(anchor_name))
            .map(|am| (i, am))
    })
}

/// The gate side a local point falls on within piece `pi` (0 if the piece has no
/// gate).
fn side_of(pi: usize, local: [i32; 3], gates: &BTreeMap<String, GateInfo>) -> u8 {
    for g in gates.values() {
        if g.piece == pi {
            return u8::from(local[g.axis] >= g.plane);
        }
    }
    0
}

/// The graph node an anchor resolves to: its carrying piece + gate side.
fn anchor_node(
    pieces: &[PiecePlacement],
    registry: &PrefabRegistry,
    anchor_name: &str,
    gates: &BTreeMap<String, GateInfo>,
) -> Option<Node> {
    let (pi, am) = anchor_piece(pieces, registry, anchor_name)?;
    Some((pi, side_of(pi, anchor_point(am), gates)))
}

/// Every connector socket of every placed piece, in world space.
fn world_sockets(pieces: &[PiecePlacement], registry: &PrefabRegistry) -> Vec<WorldSocket> {
    let mut out = Vec::new();
    for (pi, p) in pieces.iter().enumerate() {
        let Some(meta) = registry.get(&p.prefab_id) else {
            continue;
        };
        for (ci, conn) in meta.connectors.iter().enumerate() {
            let Some(f) = Facing::parse(&conn.facing) else {
                continue;
            };
            let t = p.rotation.transform(conn.local_pos);
            out.push(WorldSocket {
                piece: pi,
                connector: ci,
                world: [p.pos[0] + t[0], p.pos[1] + t[1], p.pos[2] + t[2]],
                facing: f.rotate(p.rotation),
            });
        }
    }
    out
}

/// Static adjacency over `(piece, side)` nodes: two mated sockets (child socket one
/// block beyond the parent, facing opposite) link their pieces' sub-nodes.
fn build_adjacency(
    sockets: &[WorldSocket],
    pieces: &[PiecePlacement],
    registry: &PrefabRegistry,
    gates: &BTreeMap<String, GateInfo>,
    severed: Option<&BTreeSet<[i32; 3]>>,
) -> BTreeMap<Node, BTreeSet<Node>> {
    // Local pos of a socket (for gate-side classification).
    let local_pos = |s: &WorldSocket| -> [i32; 3] {
        registry
            .get(&pieces[s.piece].prefab_id)
            .and_then(|m| m.connectors.get(s.connector))
            .map(|c| c.local_pos)
            .unwrap_or([0, 0, 0])
    };
    let mut adj: BTreeMap<Node, BTreeSet<Node>> = BTreeMap::new();
    for a in sockets {
        let a_next = [
            a.world[0] + a.facing.unit()[0],
            a.world[1] + a.facing.unit()[1],
            a.world[2] + a.facing.unit()[2],
        ];
        for b in sockets {
            if a.piece == b.piece {
                continue;
            }
            if b.world == a_next && b.facing == a.facing.opposite() {
                // A doorway severed by `rewire-socket sealed` (spec-0017)
                // is walled on both planes — no edge.
                if severed.is_some_and(|s| s.contains(&a.world) || s.contains(&b.world)) {
                    continue;
                }
                let na = (a.piece, side_of(a.piece, local_pos(a), gates));
                let nb = (b.piece, side_of(b.piece, local_pos(b), gates));
                adj.entry(na).or_default().insert(nb);
                adj.entry(nb).or_default().insert(na);
            }
        }
    }
    adj
}

/// BFS reachability from `spawn` to `target` over static edges plus the cut-edge of
/// every gate whose piece is in `open` (its two sides become connected).
fn reachable(
    spawn: Node,
    target: Node,
    adj: &BTreeMap<Node, BTreeSet<Node>>,
    gates: &BTreeMap<String, GateInfo>,
    open: &BTreeSet<usize>,
) -> bool {
    let mut seen: BTreeSet<Node> = BTreeSet::new();
    let mut queue: VecDeque<Node> = VecDeque::new();
    seen.insert(spawn);
    queue.push_back(spawn);
    while let Some(n) = queue.pop_front() {
        if n == target {
            return true;
        }
        if let Some(neis) = adj.get(&n) {
            for &m in neis {
                if seen.insert(m) {
                    queue.push_back(m);
                }
            }
        }
        // Open gate cut-edge on this piece connects its two sides.
        if open.contains(&n.0) && gates.values().any(|g| g.piece == n.0) {
            let other = (n.0, 1 - n.1);
            if seen.insert(other) {
                queue.push_back(other);
            }
        }
    }
    seen.contains(&target)
}
