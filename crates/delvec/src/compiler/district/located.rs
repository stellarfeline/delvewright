//! **The located rule** (spec-0103 §7.3): every object a district's fragment
//! carries stands in that district, and every reference it makes resolves to
//! the whole or to its own district — never to a sibling (`DW1004`).
//!
//! # How an object is located, and why one walk does it
//!
//! A quest, an NPC, a trigger, a wave, an actor, an assembly, a killing volume
//! and a pulse each reach the map through names: an anchor (`anchor/…`, a
//! station, a mark's anchor, the centre of an anchor-centred region), a place
//! (`node/…`), a plan region (`region/…`). The rule is the same for every one of
//! them, so it is ONE walk over the object as the fragment spells it — every
//! string value and every object key, at every depth — rather than a walker per
//! object class that the next anchor-bearing field would escape. A name the
//! derivation does not provide is not judged here: `DW0112` already refuses it,
//! and this code is not raised on top of it.
//!
//! The same walk finds the joins. A string that is an id a **sibling** fragment
//! declares (a quest, an objective, an NPC, a dialogue tree, a trigger, a wave,
//! an actor, an assembly, a killing volume, a pulse) is a reference across a
//! boundary, and so is a `flag/…` two fragments both write: the thing that joins
//! two districts belongs to the whole, and the message names the whole's
//! document to move it to. **The whole may name anything**; the direction of
//! the rule is the direction of ownership.
//!
//! The finer rows of the table: a fragment's quest is the plan's quest of the
//! same district; a dialogue tree is referenced only from quests of its own
//! fragment; a station is declared on a place of the district; a detail row's
//! place, and a design row's place stem, are places of the district.
//!
//! **Recorded debt**: a region is located by its centre anchor. The spec's
//! "every cell inside the district's claims" needs the region's cells, which
//! need the derivation's anchor positions — a build, not a validation.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{Campaign, Diagnostic};
use serde_json::Value;

use super::fragment::{Fragment, Fragments};
use super::{DW_OUTSIDE_DISTRICT, Partition};

/// What the located rule examined, stated on every run of a campaign that
/// declares districts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocatedBinding {
    /// Districts the graph declares.
    pub districts: usize,
    /// Fragment files read.
    pub fragments: usize,
    /// Objects those fragments carry.
    pub objects: usize,
    /// Map names (anchors, places, regions) the walk resolved.
    pub names: usize,
    /// Objects refused.
    pub refused: usize,
}

impl LocatedBinding {
    /// The binding line, zeroes included.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "district fragments: {} fragment file(s) of {} declared district(s), {} object(s) \
             located, {} map name(s) resolved, {} object(s) refused (DW1004).",
            self.fragments, self.districts, self.objects, self.names, self.refused
        )
    }
}

/// Every string in a JSON value, keys included, with the pointer it sits at.
fn strings<'a>(v: &'a Value, at: String, out: &mut Vec<(String, &'a str)>) {
    match v {
        Value::String(s) => out.push((at, s.as_str())),
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                strings(x, format!("{at}/{i}"), out);
            }
        }
        Value::Object(o) => {
            for (k, x) in o {
                let esc = k.replace('~', "~0").replace('/', "~1");
                out.push((format!("{at}/{esc}"), k.as_str()));
                strings(x, format!("{at}/{esc}"), out);
            }
        }
        _ => {}
    }
}

/// The ids a fragment declares — what a sibling may not name.
fn declared_ids(f: &Fragment) -> Vec<String> {
    let mut out = Vec::new();
    for it in &f.items {
        match it.list.as_str() {
            // A tree is keyed by its NPC, and the tree's own rule (referenced
            // only from its fragment's quests) is judged on the tree.
            "dialogues" => {}
            "nodes" => {
                for s in it
                    .value
                    .get("stations")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let Some(a) = s.get("anchor").and_then(Value::as_str) {
                        out.push(a.to_string());
                    }
                }
            }
            "details" | "references" => {}
            _ => {
                if let Some(id) = it.value.get("id").and_then(Value::as_str) {
                    out.push(id.to_string());
                }
                if it.list == "quests" {
                    for o in it
                        .value
                        .get("objectives")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        if let Some(id) = o.get("id").and_then(Value::as_str) {
                            out.push(id.to_string());
                        }
                    }
                }
            }
        }
    }
    out
}

/// Every `flag/…` a fragment writes or reads.
fn flags_of(f: &Fragment) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for it in &f.items {
        let mut all = Vec::new();
        strings(&it.value, String::new(), &mut all);
        for (_, s) in all {
            if s.starts_with("flag/") {
                out.insert(s.to_string());
            }
        }
    }
    out
}

/// The npc ids every stage-5 quest of the campaign mentions, by quest id.
fn npcs_by_quest(c: &Campaign) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    for q in &c.quests.content.quests {
        let v = serde_json::to_value(q).unwrap_or(Value::Null);
        let mut all = Vec::new();
        strings(&v, String::new(), &mut all);
        out.insert(
            q.id.as_str().to_string(),
            all.into_iter()
                .filter(|(_, s)| s.starts_with("npc/"))
                .map(|(_, s)| s.to_string())
                .collect(),
        );
    }
    out
}

/// **Judge every fragment against the located rule.** Returns the refusals and
/// what was examined. Empty for a campaign with no fragment.
#[must_use]
pub fn check(c: &Campaign, fragments: &Fragments) -> (Vec<Diagnostic>, LocatedBinding) {
    let partition = Partition::of(c);
    let mut d: Vec<Diagnostic> = fragments.deferred.clone();
    let mut b = LocatedBinding {
        districts: partition.ids.len(),
        fragments: fragments.read.len(),
        ..LocatedBinding::default()
    };
    if fragments.read.is_empty() {
        return (d, b);
    }
    // Which district declares each id.
    let mut owner: BTreeMap<String, String> = BTreeMap::new();
    for f in &fragments.read {
        for id in declared_ids(f) {
            owner.entry(id).or_insert_with(|| f.district.clone());
        }
    }
    // Which districts write each flag.
    let mut flag_districts: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for f in &fragments.read {
        for fl in flags_of(f) {
            flag_districts
                .entry(fl)
                .or_default()
                .insert(f.district.clone());
        }
    }
    // Which fragment wrote each stage-5 quest.
    let mut quest_home: BTreeMap<String, Option<String>> = BTreeMap::new();
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        quest_home.insert(
            q.id.as_str().to_string(),
            fragments
                .origin("quests", "quests", i)
                .map(|(d, _)| d.to_string()),
        );
    }
    let mentions = npcs_by_quest(c);
    let plan_district: BTreeMap<&str, Option<&str>> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.district.as_ref().map(|d| d.as_str())))
        .collect();
    let nodes: BTreeSet<&str> = c
        .layout_graph
        .iter()
        .flat_map(|g| g.content.nodes.iter().map(|n| n.id.0.as_str()))
        .collect();

    for f in &fragments.read {
        let here = f.district.as_str();
        let doc = f.path();
        for it in &f.items {
            b.objects += 1;
            let at = format!("/content/{}/{}", it.list, it.local);
            let before = d.len();
            let mut push = |path: String, m: String| {
                d.push(Diagnostic::error(
                    DW_OUTSIDE_DISTRICT,
                    f.stage_label(),
                    path,
                    m,
                ));
            };
            match it.list.as_str() {
                "nodes" => {
                    let id = it.value.get("id").and_then(Value::as_str).unwrap_or("");
                    if nodes.contains(id) {
                        let theirs = partition.of_node(id);
                        if theirs != Some(here) {
                            push(
                                format!("{at}/id"),
                                format!(
                                    "`{doc}` declares stations on `{id}`, a place of {owner}, not \
                                     of `{here}`. A district declares stations only on its own \
                                     places: move them to {fix}.",
                                    owner = theirs
                                        .map_or("no district".to_string(), |t| format!("`{t}`")),
                                    fix = theirs.map_or(
                                        "the whole's `layout-graph.json`".to_string(),
                                        |t| format!(
                                            "`districts/{}/layout-graph.json`",
                                            super::dir_of(t)
                                        )
                                    ),
                                ),
                            );
                        }
                    }
                }
                "details" => {
                    let place = it.value.get("place").and_then(Value::as_str).unwrap_or("");
                    let theirs = partition.of_node(place);
                    if theirs != Some(here) {
                        push(
                            format!("{at}/place"),
                            format!(
                                "`{doc}` carries the detail row of `{place}`, a place of {owner}. \
                                 A place's row stands in the fragment of the district the place \
                                 is in — `delvec detail` writes it there itself; move the row.",
                                owner =
                                    theirs.map_or("no district".to_string(), |t| format!("`{t}`")),
                            ),
                        );
                    }
                }
                "references" => {
                    let name = it.value.get("name").and_then(Value::as_str).unwrap_or("");
                    let stem = name.split('/').nth(1).unwrap_or("");
                    let place = format!("node/{stem}");
                    let theirs = partition.of_node(&place);
                    if theirs != Some(here) {
                        push(
                            format!("{at}/name"),
                            format!(
                                "`{doc}` records `{name}`, an image of `{place}`, which is a place \
                                 of {owner}. A district records the images of its own places; \
                                 move the row (and the image) to the district the place is in.",
                                owner = theirs.map_or(
                                    "no district (the graph declares no such place in one)"
                                        .to_string(),
                                    |t| format!("`{t}`")
                                ),
                            ),
                        );
                    }
                }
                list => {
                    // A quest is the plan's quest of this district.
                    if list == "quests" {
                        let id = it.value.get("id").and_then(Value::as_str).unwrap_or("");
                        match plan_district.get(id) {
                            Some(Some(p)) if *p == here => {}
                            Some(p) => push(
                                format!("{at}/id"),
                                format!(
                                    "`{doc}` expands `{id}`, which the quest plan assigns to {who}. \
                                     A district expands only the quests `quest-plan.json` assigns \
                                     to it (`district: \"{here}\"`); a quest with no district is \
                                     the whole's, expanded in the whole's `quests.json`.",
                                    who = p.map_or(
                                        "the whole (no `district`)".to_string(),
                                        |p| format!("`{p}`")
                                    ),
                                ),
                            ),
                            // An unplanned quest is `DW0151`'s, not this code's.
                            None => {}
                        }
                    }
                    // A dialogue tree is referenced only from quests of its fragment.
                    if list == "dialogues"
                        && let Some(npc) = it.value.get("npc").and_then(Value::as_str)
                    {
                        let outside: Vec<String> = mentions
                            .iter()
                            .filter(|(_, ns)| ns.contains(npc))
                            .filter(|(q, _)| {
                                quest_home.get(*q).cloned().flatten().as_deref() != Some(here)
                            })
                            .map(|(q, _)| q.clone())
                            .collect();
                        if !outside.is_empty() {
                            push(
                                format!("{at}/npc"),
                                format!(
                                    "`{doc}` carries the dialogue tree of `{npc}`, and quests outside \
                                     `{here}` name that NPC: {list}. A district's dialogue tree is \
                                     referenced only from its own quests; a tree two districts' \
                                     quests reach is the whole's — move it to the whole's \
                                     `dialogue.json`.",
                                    list = super::ticked(&outside),
                                ),
                            );
                        }
                    }
                    let mut all = Vec::new();
                    strings(&it.value, at.clone(), &mut all);
                    let mut said: BTreeSet<String> = BTreeSet::new();
                    for (path, s) in all {
                        let mut reason: Option<String> = None;
                        if s.contains("anchor/") && partition.knows_anchor(s) {
                            b.names += 1;
                            match partition.of_anchor(s) {
                                Some(t) if t == here => {}
                                Some(t) => {
                                    reason = Some(format!(
                                        "names `{s}`, which stands in `{t}`, not in `{here}`"
                                    ));
                                }
                                None => {
                                    reason = Some(format!(
                                        "names `{s}`, which stands on a boundary of `{here}` and \
                                         belongs to the whole"
                                    ));
                                }
                            }
                        } else if s.starts_with("node/") && nodes.contains(s) {
                            b.names += 1;
                            match partition.of_node(s) {
                                Some(t) if t == here => {}
                                Some(t) => {
                                    reason = Some(format!(
                                        "names the place `{s}`, which is in `{t}`, not in `{here}`"
                                    ));
                                }
                                None => {}
                            }
                        } else if s.starts_with("region/") {
                            b.names += 1;
                            reason = Some(format!(
                                "names the plan's region `{s}`, which no district owns"
                            ));
                        } else if let Some(t) = owner.get(s)
                            && t != here
                        {
                            reason = Some(format!(
                                "names `{s}`, which `{t}` declares — a reference into a sibling \
                                 district"
                            ));
                        } else if s.starts_with("flag/")
                            && let Some(ds) = flag_districts.get(s)
                            && ds.len() > 1
                        {
                            let others: Vec<String> =
                                ds.iter().filter(|x| *x != here).cloned().collect();
                            reason = Some(format!(
                                "writes or reads `{s}`, which {} also writes or reads — a flag \
                                 that joins two districts",
                                super::ticked(&others)
                            ));
                        }
                        let Some(reason) = reason else {
                            continue;
                        };
                        if !said.insert(reason.clone()) {
                            continue;
                        }
                        push(
                            path,
                            format!(
                                "`{doc}`: {kind} {reason}. Every object a district's fragment \
                                 carries stands in its district and refers only to the whole or \
                                 to its own district. A thing that crosses a boundary is the \
                                 whole's: move it to the whole's `{whole}` (the planner's \
                                 document), or ask the planner to.",
                                kind = it
                                    .value
                                    .get("id")
                                    .and_then(Value::as_str)
                                    .or_else(|| it.value.get("npc").and_then(Value::as_str))
                                    .map_or(format!("{} {}", it.list, it.local), |id| format!(
                                        "`{id}`"
                                    )),
                                whole = f.file,
                            ),
                        );
                    }
                }
            }
            if d.len() > before {
                b.refused += 1;
            }
        }
    }
    (d, b)
}
