//! **The handover packet** (spec-0103 §6): what one worker knows — the
//! campaign's concept, its district's place in the whole, and the contract at
//! every boundary with a neighbour.
//!
//! Derived from the documents on every invocation and **an input to nothing**:
//! `validate`, `detail` and the battery recompute every obligation from the
//! plan, so a stale packet has no vector into the build. Nothing in it is typed
//! by hand; a planner that wants to say more to a worker edits a document
//! (`districts[].note`, `nodes[].note`, `seams[].form`) and derives again.
//!
//! `places[]` carries each place's allocation exactly as `delvec allocation`
//! prints it — the same function, serialised the same way.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use delvewright_dsl::{Campaign, Edge, NodeId};
use serde_json::{Value, json};

use super::fragment::Fragments;
use super::writes::WriteSet;
use super::{Partition, dir_of, stem_of};

/// Which wave a packet is for (spec-0103 §8.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wave {
    /// Step 5: the district's quests, dialogue, NPCs and stations.
    Story,
    /// Step 9: the district's places detailed.
    Build,
    /// A finding routed to the district, verbatim.
    Repair(String),
}

impl Wave {
    fn json(&self) -> Value {
        match self {
            Wave::Story => json!({"kind": "story"}),
            Wave::Build => json!({"kind": "build"}),
            Wave::Repair(f) => json!({"kind": "repair", "finding": f}),
        }
    }
}

/// What a packet is derived against beyond the documents.
#[derive(Debug, Clone)]
pub struct Context<'a> {
    /// The campaign directory.
    pub campaign_dir: &'a Path,
    /// The prefab directory the worker details into, repository-relative.
    pub prefabs_rel: &'a str,
    /// The campaign directory, repository-relative.
    pub campaign_rel: &'a str,
    /// The wave.
    pub wave: Wave,
    /// The campaign branch's commit the worktree is cut from, when known.
    pub baseline: Option<String>,
}

/// A derived packet: the document and the images it names (campaign-relative
/// paths to copy under `images/`).
#[derive(Debug, Clone)]
pub struct Packet {
    /// `packet.json`.
    pub doc: Value,
    /// Every image the packet names, campaign-relative.
    pub images: Vec<String>,
}

fn word_in(hay: &str, word: &str) -> bool {
    hay.match_indices(word).any(|(i, _)| {
        let before = hay[..i].chars().next_back();
        let after = hay[i + word.len()..].chars().next();
        let edge = |c: Option<char>| c.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '-'));
        edge(before) && edge(after)
    })
}

fn strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
        Value::Object(o) => {
            for (k, x) in o {
                out.push(k.clone());
                strings(x, out);
            }
        }
        _ => {}
    }
}

/// The districts a JSON value's names stand in.
fn districts_named(p: &Partition, v: &Value) -> BTreeSet<String> {
    let mut all = Vec::new();
    strings(v, &mut all);
    all.iter()
        .filter_map(|s| {
            if s.contains("anchor/") {
                p.of_anchor(s).map(str::to_string)
            } else if s.starts_with("node/") {
                p.of_node(s).map(str::to_string)
            } else {
                None
            }
        })
        .collect()
}

/// The names in a JSON value that stand in `district`.
fn names_in(p: &Partition, v: &Value, district: &str) -> Vec<String> {
    let mut all = Vec::new();
    strings(v, &mut all);
    let mut out: Vec<String> = all
        .into_iter()
        .filter(|s| {
            (s.contains("anchor/") && p.of_anchor(s) == Some(district))
                || (s.starts_with("node/") && p.of_node(s) == Some(district))
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Items given in one district and required in another: item → (given in,
/// required in, giving quests, requiring quests).
type ItemFlow = BTreeMap<
    String,
    (
        BTreeSet<String>,
        BTreeSet<String>,
        BTreeSet<String>,
        BTreeSet<String>,
    ),
>;

/// Where each item is given and where it is required, by the districts of the
/// objectives the effects and the requirements hang on.
fn item_flow(c: &Campaign, p: &Partition) -> ItemFlow {
    let mut flow: ItemFlow = BTreeMap::new();
    for q in &c.quests.content.quests {
        let qv = serde_json::to_value(q).unwrap_or(Value::Null);
        let qid = q.id.as_str().to_string();
        let objectives = qv
            .get("objectives")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut obj_district: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for o in &objectives {
            let id = o
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let ds = districts_named(p, o);
            // An objective that requires an item, by any spelling the
            // objective carries it in.
            let mut items = Vec::new();
            collect_items(o, false, &mut items);
            for it in items {
                let e = flow.entry(it).or_default();
                e.1.extend(ds.iter().cloned());
                e.3.insert(qid.clone());
            }
            obj_district.insert(id, ds);
        }
        // An item given by an effect that fires on an objective is given where
        // that objective stands.
        if let Some(on) = qv.get("on_objective_complete").and_then(Value::as_object) {
            for (obj, effects) in on {
                let mut given = Vec::new();
                collect_given(effects, &mut given);
                for it in given {
                    let e = flow.entry(it).or_default();
                    e.0.extend(obj_district.get(obj).cloned().unwrap_or_default());
                    e.2.insert(qid.clone());
                }
            }
        }
        if let Some(effects) = qv.get("on_complete") {
            let mut given = Vec::new();
            collect_given(effects, &mut given);
            let last: BTreeSet<String> = objectives
                .last()
                .map(|o| districts_named(p, o))
                .unwrap_or_default();
            for it in given {
                let e = flow.entry(it).or_default();
                e.0.extend(last.iter().cloned());
                e.2.insert(qid.clone());
            }
        }
    }
    flow
}

/// Every item a `give-item` effect hands, at any depth.
fn collect_given(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Array(a) => a.iter().for_each(|x| collect_given(x, out)),
        Value::Object(o) => {
            if o.get("verb").and_then(Value::as_str) == Some("give-item")
                && let Some(item) = o.get("item").and_then(Value::as_str)
            {
                out.push(item.to_string());
            }
            o.values().for_each(|x| collect_given(x, out));
        }
        _ => {}
    }
}

/// Every item an objective requires: an `item` field outside a nested effect.
fn collect_items(v: &Value, in_effect: bool, out: &mut Vec<String>) {
    match v {
        Value::Array(a) => a.iter().for_each(|x| collect_items(x, in_effect, out)),
        Value::Object(o) => {
            let effect = in_effect || o.contains_key("verb");
            if !effect {
                for key in ["item", "requires_item"] {
                    if let Some(item) = o.get(key).and_then(Value::as_str) {
                        out.push(item.to_string());
                    }
                }
            }
            o.values().for_each(|x| collect_items(x, effect, out));
        }
        _ => {}
    }
}

/// **The connection contract for one boundary edge** (spec-0103 §6.3).
fn contract(
    c: &Campaign,
    p: &Partition,
    edge: &Edge,
    allocations: &BTreeMap<String, Value>,
    flow: &ItemFlow,
) -> Value {
    let (a, b) = (edge.a().0.clone(), edge.b().0.clone());
    let (da, db) = (
        p.of_node(&a).unwrap_or("whole").to_string(),
        p.of_node(&b).unwrap_or("whole").to_string(),
    );
    let id = edge.id().0.clone();
    let seam_doc = c.site_plan.as_ref().and_then(|s| {
        s.content
            .seams
            .iter()
            .find(|x| x.edge.0 == id)
            .map(|x| (x, serde_json::to_value(x).unwrap_or(Value::Null)))
    });
    let alloc_seam = |place: &str| -> Option<Value> {
        allocations
            .get(place)?
            .get("seams")?
            .as_array()?
            .iter()
            .find(|s| s.get("edge").and_then(Value::as_str) == Some(id.as_str()))
            .cloned()
    };
    let floor = |place: &str| -> Option<i64> {
        allocations
            .get(place)?
            .get("ground")?
            .get("floor_y")?
            .as_i64()
    };
    let form = alloc_seam(&a)
        .or_else(|| alloc_seam(&b))
        .and_then(|s| s.get("form").cloned())
        .unwrap_or(Value::Null);
    let opening = seam_doc.as_ref().map_or(Value::Null, |(_, v)| {
        v.get("opening")
            .cloned()
            .or_else(|| v.get("contact").cloned().map(|c| json!({"contact": c})))
            .unwrap_or(Value::Null)
    });
    let (fa, fb) = (floor(&a), floor(&b));
    let height = match (fa, fb) {
        (Some(x), Some(y)) => json!({
            "sill_y": x.max(y),
            "rise": (x - y).abs(),
            "stair_in": seam_doc.as_ref().and_then(|(s, _)| s.stair_in.as_ref().map(|n| n.0.clone())),
        }),
        _ => Value::Null,
    };
    let (landing, facing) = match (&seam_doc, alloc_seam(&a).or_else(|| alloc_seam(&b))) {
        (Some((s, _)), Some(al)) => {
            let v = s.face.vector();
            let cells = al
                .get("cells")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let corner = |k: usize| -> Option<[i64; 3]> {
                let c = cells.get(k)?.as_array()?;
                Some([
                    c.first()?.as_i64()?,
                    c.get(1)?.as_i64()?,
                    c.get(2)?.as_i64()?,
                ])
            };
            let landing = match (corner(0), corner(1)) {
                (Some(lo), Some(hi)) => {
                    let y = lo[1].min(hi[1]);
                    let row = |sign: i64| {
                        json!([
                            [lo[0] + sign * v[0], y, lo[2] + sign * v[2]],
                            [hi[0] + sign * v[0], y, hi[2] + sign * v[2]],
                        ])
                    };
                    json!({ a.clone(): row(-1), b.clone(): row(1) })
                }
                _ => Value::Null,
            };
            (landing, json!({"face_of_a": s.face, "into_b": s.face}))
        }
        _ => (Value::Null, Value::Null),
    };
    let gating = json!({
        "class": edge.class(),
        "gating": edge.gating(),
        "opens_from": match edge {
            Edge::Barred { opens_from, .. } => serde_json::to_value(opens_from).unwrap_or(Value::Null),
            _ => Value::Null,
        },
        "one_way": edge.direction(),
        "shortcut": edge.shortcut(),
    });
    // What passes.
    let path: Vec<String> = c
        .layout_graph
        .iter()
        .flat_map(|g| g.content.critical_path.iter().map(|n| n.0.clone()))
        .collect();
    let mut party = Vec::new();
    for w in path.windows(2) {
        if (w[0] == a && w[1] == b) || (w[0] == b && w[1] == a) {
            party.push(json!({"from": w[0], "to": w[1]}));
        }
    }
    let pair: BTreeSet<String> = [da.clone(), db.clone()].into_iter().collect();
    let mut npcs = Vec::new();
    for n in &c.npcs.content.npcs {
        let mut ds = districts_named(p, &serde_json::to_value(n).unwrap_or(Value::Null));
        let mut quests = BTreeSet::new();
        for q in &c.quests.content.quests {
            if let Some(cast) = q.cast.get(&n.id) {
                let cd = districts_named(p, &serde_json::to_value(cast).unwrap_or(Value::Null));
                if !cd.is_empty() {
                    quests.insert(q.id.as_str().to_string());
                }
                ds.extend(cd);
            }
        }
        if pair.is_subset(&ds) {
            npcs.push(json!({"npc": n.id.as_str(), "quests": quests}));
        }
    }
    let mut items = Vec::new();
    for (item, (given, required, givers, needers)) in flow {
        let crosses = (given.contains(&da) && required.contains(&db))
            || (given.contains(&db) && required.contains(&da));
        if crosses {
            items.push(json!({
                "item": item,
                "given_in": given,
                "required_in": required,
                "given_by": givers,
                "required_by": needers,
            }));
        }
    }
    let atmos = |place: &str| -> Value {
        c.site_plan
            .as_ref()
            .and_then(|s| s.content.boxes.iter().find(|bx| bx.node.0 == place))
            .and_then(|bx| bx.atmosphere.as_ref().map(|a| json!(a.as_str())))
            .unwrap_or(Value::Null)
    };
    let (aa, ab) = (atmos(&a), atmos(&b));
    let mut sound = Vec::new();
    for pulse in &c.quests.content.pulses {
        let at = p.of_anchor(pulse.at.anchor.as_str()).map(str::to_string);
        let heard = pulse
            .heard
            .place
            .as_deref()
            .and_then(|pl| p.of_node(pl))
            .or_else(|| {
                pulse
                    .heard
                    .region
                    .as_ref()
                    .and_then(|r| p.of_anchor(r.anchor.as_str()))
            })
            .map(str::to_string);
        if let (Some(x), Some(y)) = (at, heard)
            && x != y
            && pair.contains(&x)
            && pair.contains(&y)
        {
            sound.push(json!({"pulse": pulse.id.as_str(), "sounds_in": x, "heard_in": y}));
        }
    }
    let mut sight = Vec::new();
    if let Some(g) = c.layout_graph.as_ref().map(|g| &g.content) {
        for e in &g.edges {
            if let Edge::Vision { id, a: va, b: vb } = e {
                let (x, y) = (p.of_node(&va.0), p.of_node(&vb.0));
                if let (Some(x), Some(y)) = (x, y)
                    && x != y
                    && pair.contains(x)
                    && pair.contains(y)
                {
                    sight.push(json!({"vision": id.0, "a": va.0, "b": vb.0}));
                }
            }
        }
    }
    if let Some(s) = &c.site_plan {
        for sl in &s.content.sightlines {
            if sl.edge.0 == id {
                sight.push(json!({"sightline": sl.edge.0, "from": sl.from, "to": sl.to}));
            }
        }
    }
    json!({
        "edge": id,
        "class": edge.class(),
        "form": form,
        "a": a,
        "b": b,
        "opening": opening,
        "height": height,
        "landing": landing,
        "facing": facing,
        "gating": gating,
        "passes": {
            "party": party,
            "npcs": npcs,
            "items": items,
            "atmosphere": {
                a.clone(): aa,
                b.clone(): ab,
                "equal": aa == ab,
            },
            "sound": sound,
            "sight": sight,
        },
    })
}

/// **Derive the packet** for `district`.
///
/// # Errors
///
/// A district the graph does not declare, or a campaign with no site plan.
pub fn derive(
    c: &Campaign,
    fragments: &Fragments,
    design: &crate::compiler::design::DesignFiles,
    district: &str,
    ctx: &Context<'_>,
    write_set: &WriteSet,
) -> Result<Packet, String> {
    let p = Partition::of(c);
    if !p.ids.iter().any(|d| d == district) {
        return Err(format!(
            "`{district}` is not a district this campaign declares (it declares {})",
            super::ticked(&p.ids)
        ));
    }
    if c.site_plan.is_none() {
        return Err(
            "the campaign carries no `site-plan.json`: a packet hands each place its allocation, \
             and there is no allocation until the whole exists"
                .to_string(),
        );
    }
    let graph = &c
        .layout_graph
        .as_ref()
        .expect("districts imply a graph")
        .content;
    let decl = graph
        .districts
        .iter()
        .find(|d| d.id.as_str() == district)
        .expect("checked above");
    let inside = |n: &str| p.of_node(n) == Some(district);
    let mut images: BTreeSet<String> = BTreeSet::new();

    // ---- campaign
    let w = &c.world.content;
    let rows: Vec<&delvewright_dsl::Reference> = c
        .design
        .iter()
        .flat_map(|d| d.content.references.iter())
        .collect();
    let file_of = |name: &str| -> Option<String> {
        design
            .images
            .iter()
            .find(|i| i.name == name)
            .map(|i| format!("design/{}", i.file))
    };
    let mut views = Vec::new();
    for r in rows
        .iter()
        .filter(|r| r.directory() == Some(delvewright_dsl::REFERENCE_DIR))
    {
        let file = file_of(&r.name);
        if let Some(f) = &file {
            images.insert(f.clone());
        }
        views.push(json!({
            "name": r.name, "shows": r.shows, "time": r.time, "weather": r.weather, "file": file,
        }));
    }
    let anchor = rows
        .iter()
        .find(|r| r.directory() == Some(delvewright_dsl::REFERENCE_DIR))
        .map(|r| {
            let sidecar = ctx
                .campaign_dir
                .join("design")
                .join(format!("{}.json", r.name));
            let side: Value = std::fs::read_to_string(&sidecar)
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(Value::Null);
            json!({
                "name": r.name,
                "file": file_of(&r.name),
                "id": side.get("id").cloned().unwrap_or(Value::Null),
                "style_contract": side
                    .get("request")
                    .and_then(|q| q.get("style_note"))
                    .cloned()
                    .unwrap_or(Value::Null),
            })
        })
        .unwrap_or(Value::Null);
    let readme = std::fs::read_to_string(ctx.campaign_dir.join("design").join("README.md")).ok();
    let campaign = json!({
        "id": c.world.campaign_id.0,
        "title": w.title,
        "theme": w.theme,
        "premise": w.premise,
        "time": w.time,
        "weather": w.weather,
        "target_minutes": w.target_minutes,
        "languages": w.languages,
        "dsl": delvewright_dsl::DSL_VERSION,
        "reference_views": views,
        "view_1": anchor,
        "design_readme": readme,
    });

    // ---- district
    let nodes: Vec<Value> = graph
        .nodes
        .iter()
        .filter(|n| inside(n.id.0.as_str()))
        .map(|n| {
            json!({
                "id": n.id.0,
                "intent": n.intent,
                "note": n.note,
                "stations": n.stations,
                "reached": n.reached,
            })
        })
        .collect();
    let edges: Vec<Value> = graph
        .edges
        .iter()
        .filter(|e| inside(e.a().0.as_str()) && inside(e.b().0.as_str()))
        .map(|e| serde_json::to_value(e).unwrap_or(Value::Null))
        .collect();
    let mut path = Vec::new();
    for (i, n) in graph.critical_path.iter().enumerate() {
        if !inside(n.0.as_str()) {
            continue;
        }
        let prev = i.checked_sub(1).and_then(|k| graph.critical_path.get(k));
        let next = graph.critical_path.get(i + 1);
        path.push(json!({
            "step": i,
            "node": n.0,
            "enters_from": prev.filter(|x| !inside(x.0.as_str())).map(|x| x.0.clone()),
            "leaves_to": next.filter(|x| !inside(x.0.as_str())).map(|x| x.0.clone()),
        }));
    }
    let beats: Vec<Value> = graph
        .beats
        .iter()
        .filter(|b| inside(b.node.0.as_str()))
        .map(|b| serde_json::to_value(b).unwrap_or(Value::Null))
        .collect();
    let district_doc = json!({
        "id": district,
        "note": decl.note,
        "nodes": nodes,
        "edges": edges,
        "critical_path": path,
        "beats": beats,
    });

    // ---- story
    let mut acts: BTreeSet<u32> = BTreeSet::new();
    let mut owed = Vec::new();
    let mut cast_ids: BTreeSet<String> = BTreeSet::new();
    for q in &c.quest_plan.content.quests {
        acts.insert(q.act);
        if q.district.as_ref().map(|d| d.as_str()) != Some(district) {
            continue;
        }
        let objectives: Vec<Value> = graph
            .beats
            .iter()
            .filter(|b| b.quest == q.id)
            .map(|b| json!({"objective": b.objective.as_str(), "node": b.node.0}))
            .collect();
        for n in &q.npcs {
            cast_ids.insert(n.as_str().to_string());
        }
        owed.push(json!({
            "id": q.id.as_str(),
            "goal": q.goal,
            "depends_on": q.depends_on,
            "npcs": q.npcs,
            "mandatory": q.mandatory,
            "act": q.act,
            "objectives": objectives,
        }));
    }
    let goals: BTreeMap<&str, &str> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.goal.as_str()))
        .collect();
    let mut whole_here = Vec::new();
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        if fragments.origin("quests", "quests", i).is_some() {
            continue;
        }
        let v = serde_json::to_value(q).unwrap_or(Value::Null);
        let mut objects = names_in(&p, &v, district);
        for b in graph
            .beats
            .iter()
            .filter(|b| b.quest == q.id && inside(b.node.0.as_str()))
        {
            objects.push(b.objective.as_str().to_string());
        }
        objects.sort();
        objects.dedup();
        if objects.is_empty() {
            continue;
        }
        whole_here.push(json!({
            "id": q.id.as_str(),
            "goal": goals.get(q.id.as_str()),
            "objects_here": objects,
        }));
    }
    let cast: Vec<Value> = c
        .npcs
        .content
        .npcs
        .iter()
        .filter(|n| cast_ids.contains(n.id.as_str()))
        .map(|n| serde_json::to_value(n).unwrap_or(Value::Null))
        .collect();
    let story = json!({
        "acts": acts,
        "owed": owed,
        "whole_here": whole_here,
        "cast": cast,
    });

    // ---- concept
    let stems: BTreeSet<String> = p
        .places_of(c, district)
        .iter()
        .map(|n| stem_of(&n.0).to_string())
        .collect();
    let mut concept = Vec::new();
    for r in &rows {
        let mine = r.place().is_some_and(|s| stems.contains(s))
            || (r.directory() != Some(delvewright_dsl::REFERENCE_DIR)
                && stems
                    .iter()
                    .any(|s| word_in(&r.shows, &format!("node/{s}")) || word_in(&r.shows, s)));
        if !mine {
            continue;
        }
        let file = file_of(&r.name);
        if let Some(f) = &file {
            images.insert(f.clone());
        }
        concept.push(json!({
            "name": r.name, "shows": r.shows, "time": r.time, "weather": r.weather, "file": file,
        }));
    }

    // ---- places: the allocation, verbatim
    let mut allocations: BTreeMap<String, Value> = BTreeMap::new();
    for a in crate::compiler::detail::allocations(c) {
        let v = serde_json::to_value(&a).map_err(|e| e.to_string())?;
        allocations.insert(a.place.clone(), v);
    }
    let places: Vec<Value> = p
        .places_of(c, district)
        .iter()
        .filter_map(|n: &NodeId| allocations.get(&n.0).cloned())
        .collect();

    // ---- neighbours
    let flow = item_flow(c, &p);
    let mut by_neighbour: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for e in &graph.edges {
        let (a, b) = (e.a().0.as_str(), e.b().0.as_str());
        let other = match (inside(a), inside(b)) {
            (true, false) => p.of_node(b),
            (false, true) => p.of_node(a),
            _ => continue,
        };
        let other = other.unwrap_or("whole").to_string();
        by_neighbour
            .entry(other)
            .or_default()
            .push(contract(c, &p, e, &allocations, &flow));
    }
    let neighbours: Vec<Value> = by_neighbour
        .into_iter()
        .map(|(d, contracts)| json!({"district": d, "contracts": contracts}))
        .collect();

    let images: Vec<String> = images.into_iter().collect();
    let doc = json!({
        "campaign": campaign,
        "district": district_doc,
        "story": story,
        "concept": concept,
        "places": places,
        "neighbours": neighbours,
        "wave": ctx.wave.json(),
        "write_set": write_set.rules(),
        "tools": {
            "prefabs": ctx.prefabs_rel,
            "campaign": ctx.campaign_rel,
            "env": "source \"$DELVEWRIGHT_ENV\"   # the creator's env.sh, the one pinned engine",
            "district_flag": format!("--district {district}"),
            "validate": format!(
                "delvec --prefabs {pre} validate {cam} --district {district}",
                pre = ctx.prefabs_rel,
                cam = if ctx.campaign_rel.is_empty() { "." } else { ctx.campaign_rel },
            ),
        },
        "baseline": ctx.baseline,
        "images": images.iter().map(|i| format!("images/{i}")).collect::<Vec<_>>(),
        "directory": format!("{}/{}", super::DISTRICTS_DIR, dir_of(district)),
    });
    Ok(Packet { doc, images })
}

/// **`PACKET.md`**, rendered from `packet.json` in the order a worker reads it.
#[must_use]
pub fn render(doc: &Value) -> String {
    let s = |v: &Value| -> String {
        match v {
            Value::String(x) => x.clone(),
            Value::Null => "—".to_string(),
            other => other.to_string(),
        }
    };
    let mut out = String::new();
    let d = &doc["district"];
    let c = &doc["campaign"];
    out.push_str(&format!(
        "# Packet: {} — {}\n\n",
        s(&d["id"]),
        s(&c["title"])
    ));
    out.push_str(
        "Derived by `delvec district packet` from the campaign's documents; nothing in it is \
         typed by hand, and nothing reads it back. If something here is wrong, the document it \
         came from is wrong: say so in your report.\n\n",
    );
    out.push_str(&format!("**Wave:** {}\n\n", s(&doc["wave"]["kind"])));
    if let Some(f) = doc["wave"].get("finding") {
        out.push_str(&format!(
            "**The finding to repair, verbatim:**\n\n```\n{}\n```\n\n",
            s(f)
        ));
    }
    out.push_str(&format!(
        "## The campaign\n\n{} — {}\n\n{}\n\nSky: {} / {}; target {} minutes; languages {}; dsl {}.\n\n",
        s(&c["title"]),
        s(&c["theme"]),
        s(&c["premise"]),
        s(&c["time"]),
        s(&c["weather"]),
        s(&c["target_minutes"]),
        s(&c["languages"]),
        s(&c["dsl"]),
    ));
    if !c["view_1"].is_null() {
        out.push_str(&format!(
            "Map view 1 (the style anchor): `{}` (file `{}`, interaction `{}`).\n\nStyle contract: {}\n\n",
            s(&c["view_1"]["name"]),
            s(&c["view_1"]["file"]),
            s(&c["view_1"]["id"]),
            s(&c["view_1"]["style_contract"]),
        ));
    }
    if let Some(r) = c["design_readme"].as_str() {
        out.push_str(&format!(
            "The design record (`design/README.md`):\n\n{r}\n\n"
        ));
    }
    out.push_str(&format!(
        "## Your district: `{}`\n\n{}\n\n### Places\n\n",
        s(&d["id"]),
        s(&d["note"])
    ));
    for n in d["nodes"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "- `{}` — {}{}{}\n",
            s(&n["id"]),
            s(&n["intent"]),
            if n["reached"] == Value::Bool(false) {
                " (scenery: built to be seen, never entered)"
            } else {
                ""
            },
            n["note"]
                .as_str()
                .map(|x| format!(": {x}"))
                .unwrap_or_default(),
        ));
    }
    out.push_str("\n### The critical path through it\n\n");
    for st in d["critical_path"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "- step {}: `{}`{}{}\n",
            s(&st["step"]),
            s(&st["node"]),
            st["enters_from"]
                .as_str()
                .map(|x| format!(" — enters from `{x}`"))
                .unwrap_or_default(),
            st["leaves_to"]
                .as_str()
                .map(|x| format!(" — leaves to `{x}`"))
                .unwrap_or_default(),
        ));
    }
    out.push_str("\n## The story you owe\n\n");
    for q in doc["story"]["owed"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "- `{}` (act {}) — {}; objectives the graph's beats name: {}\n",
            s(&q["id"]),
            s(&q["act"]),
            s(&q["goal"]),
            q["objectives"]
                .as_array()
                .map(|o| o
                    .iter()
                    .map(|x| format!("`{}` at `{}`", s(&x["objective"]), s(&x["node"])))
                    .collect::<Vec<_>>()
                    .join(", "))
                .filter(|x| !x.is_empty())
                .unwrap_or_else(|| "none".to_string()),
        ));
    }
    out.push_str("\n### The whole's quests with objects in your places (build what they need; write none of them)\n\n");
    for q in doc["story"]["whole_here"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "- `{}` — {}: {}\n",
            s(&q["id"]),
            s(&q["goal"]),
            s(&q["objects_here"])
        ));
    }
    out.push_str("\n## Every boundary, and its contract\n\n");
    for nb in doc["neighbours"].as_array().into_iter().flatten() {
        out.push_str(&format!("### With `{}`\n\n", s(&nb["district"])));
        for k in nb["contracts"].as_array().into_iter().flatten() {
            out.push_str(&format!(
                "- `{}` ({}) `{}` ↔ `{}` — form: {}\n  - opening {}; height {}; landing {}; facing {}\n  - gating {}\n  - passes: {}\n",
                s(&k["edge"]), s(&k["class"]), s(&k["a"]), s(&k["b"]), s(&k["form"]),
                s(&k["opening"]), s(&k["height"]), s(&k["landing"]), s(&k["facing"]),
                s(&k["gating"]), s(&k["passes"]),
            ));
        }
        out.push('\n');
    }
    out.push_str("## Concept images of your places\n\n");
    for i in doc["concept"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "- `{}` — {} ({})\n",
            s(&i["name"]),
            s(&i["shows"]),
            s(&i["file"])
        ));
    }
    out.push_str("\n## Your allocations\n\n`packet.json` `places[]` carries each place's allocation exactly as `delvec allocation` prints it.\n\n");
    out.push_str("## What you may write\n\nThe pre-commit hook refuses anything else (`delvec district writes`, DW1008):\n\n");
    for r in doc["write_set"].as_array().into_iter().flatten() {
        out.push_str(&format!("- `{}`\n", s(r)));
    }
    out.push_str(&format!(
        "\n## Tools\n\n- prefabs: `{}`\n- `{}`\n- {}\n- baseline: `{}`\n",
        s(&doc["tools"]["prefabs"]),
        s(&doc["tools"]["validate"]),
        s(&doc["tools"]["env"]),
        s(&doc["baseline"]),
    ));
    out
}
