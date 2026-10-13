//! **District fragments of the stage documents** (spec-0103 §7.1).
//!
//! An object's owner is a property of **where it is written**: a campaign
//! directory may hold `districts/<x>/` for each declared district, and in it
//! fragments of six stage documents ([`super::FRAGMENTS`]). A fragment is the
//! same stage's envelope with the same four keys; its `content` carries only
//! the lists the table names for it, and every other key is the whole's
//! (`DW1005`). A layout-graph fragment's `nodes[]` carry `id` and `stations[]`
//! only — the stations a worker declares on its own places — and a design
//! fragment's rows are the per-place images under `design/places/<stem>/`.
//!
//! The loader reads `districts/*/` in **sorted district order** and unions each
//! fragment's lists into the stage document's after the whole's own, so every
//! validation that runs over a list runs over the union: an id declared twice
//! across a fragment and the whole, or across two fragments, is the duplicate it
//! already is. Every fragment's bytes are a manifest input. The union order is a
//! function of the files' names and contents, so the same objects yield the same
//! bytes whether they stand in the whole or in a fragment (ADR-0006).
//!
//! **A diagnostic about an object a fragment wrote is relocated to the
//! fragment**: the union is the engine's working form, and a pointer into it —
//! `/content/quests/7` — names an object of a file the author never wrote. The
//! provenance recorded here turns it back into `districts/<x>/quests` and the
//! fragment's own index ([`Fragments::relocate`]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use delvewright_dsl::{Diagnostic, RawCampaign, codes};
use serde_json::Value;

use super::{DISTRICT_PREFIX, DISTRICTS_DIR, DW_WHOLE_OWNS, FragmentKind, fragment_kind};

/// One object a fragment carries, kept as written.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// The `content` list it is in (`quests`, `npcs`, …).
    pub list: String,
    /// Its index in the fragment's list.
    pub local: usize,
    /// The object, as the fragment spells it.
    pub value: Value,
}

/// One fragment file, read.
#[derive(Debug, Clone, PartialEq)]
pub struct Fragment {
    /// The district id (`district/<x>`).
    pub district: String,
    /// The file name (`quests.json`, …).
    pub file: &'static str,
    /// Every object it carries, list by list in the kind's order.
    pub items: Vec<Item>,
}

impl Fragment {
    /// `districts/<x>/<file>`.
    #[must_use]
    pub fn path(&self) -> String {
        format!(
            "{DISTRICTS_DIR}/{}/{}",
            super::dir_of(&self.district),
            self.file
        )
    }

    /// `districts/<x>/<stage>` — what a relocated diagnostic names as its stage.
    #[must_use]
    pub fn stage_label(&self) -> String {
        format!(
            "{DISTRICTS_DIR}/{}/{}",
            super::dir_of(&self.district),
            self.file.trim_end_matches(".json")
        )
    }
}

/// Where a run of a fragment's items landed in the union.
#[derive(Debug, Clone, PartialEq)]
struct Span {
    district: String,
    start: usize,
    len: usize,
}

/// Where a fragment's stations landed on one node of the union.
#[derive(Debug, Clone, PartialEq)]
struct StationSpan {
    district: String,
    /// The fragment node's index in its `nodes[]`.
    local_node: usize,
    start: usize,
    len: usize,
}

/// **Every fragment of a campaign directory, and where its objects landed.**
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fragments {
    /// Every fragment, in sorted district order, then file order.
    pub read: Vec<Fragment>,
    /// The `districts/<x>/` directories present, sorted.
    pub directories: Vec<String>,
    /// `(stage, list)` → the spans fragments contributed to the union's list.
    spans: BTreeMap<(String, String), Vec<Span>>,
    /// The union's node order (`layout-graph` `nodes[]` ids).
    node_order: Vec<String>,
    /// node id → (the whole's own station count, the fragments' spans).
    stations: BTreeMap<String, (usize, Vec<StationSpan>)>,
    /// Refusals that stop the parse: a fragment the engine cannot union.
    pub refusals: Vec<Diagnostic>,
    /// Refusals judged after the parse, beside the located rule: a station
    /// declared on a node the graph does not have.
    pub deferred: Vec<Diagnostic>,
}

impl Fragments {
    /// True when the campaign directory holds no fragment at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.read.is_empty() && self.directories.is_empty()
    }

    /// The fragments of one district.
    pub fn of_district<'a>(&'a self, district: &'a str) -> impl Iterator<Item = &'a Fragment> {
        self.read.iter().filter(move |f| f.district == district)
    }

    /// The district a union index of `(stage, list)` came from, with its index
    /// in that fragment's list — `None` for an object of the whole.
    #[must_use]
    pub fn origin(&self, stage: &str, list: &str, index: usize) -> Option<(&str, usize)> {
        self.spans
            .get(&(stage.to_string(), list.to_string()))?
            .iter()
            .find(|s| index >= s.start && index < s.start + s.len)
            .map(|s| (s.district.as_str(), index - s.start))
    }

    /// **Point a diagnostic at the fragment that wrote its object**, when it
    /// names one by a union pointer: `quests` `/content/quests/7/…` becomes
    /// `districts/<x>/quests` `/content/quests/<local>/…`. Returns the district
    /// it was relocated into. A diagnostic already naming a fragment, or naming
    /// the whole's object, is left as it is.
    pub fn relocate(&self, d: &mut Diagnostic) -> Option<String> {
        if let Some(rest) = d.stage.strip_prefix(&format!("{DISTRICTS_DIR}/")) {
            let dir = rest.split('/').next().unwrap_or("");
            return Some(format!("{DISTRICT_PREFIX}{dir}"));
        }
        let rest = d.path.strip_prefix("/content/")?;
        let mut parts = rest.splitn(3, '/');
        let list = parts.next()?;
        let index: usize = parts.next()?.parse().ok()?;
        let tail = parts.next();
        // A station a fragment declared, on a node of the whole's graph.
        if d.stage == "layout-graph" && list == "nodes" {
            let tail = tail?;
            let mut t = tail.splitn(3, '/');
            if t.next()? != "stations" {
                return None;
            }
            let j: usize = t.next()?.parse().ok()?;
            let more = t.next();
            let node = self.node_order.get(index)?;
            let (own, spans) = self.stations.get(node)?;
            if j < *own {
                return None;
            }
            let s = spans.iter().find(|s| j >= s.start && j < s.start + s.len)?;
            let dir = super::dir_of(&s.district);
            d.stage = format!("{DISTRICTS_DIR}/{dir}/layout-graph");
            d.path = format!(
                "/content/nodes/{}/stations/{}{}",
                s.local_node,
                j - s.start,
                more.map(|m| format!("/{m}")).unwrap_or_default()
            );
            return Some(s.district.clone());
        }
        let (district, local) = self.origin(&d.stage, list, index)?;
        let district = district.to_string();
        let dir = super::dir_of(&district).to_string();
        d.path = format!(
            "/content/{list}/{local}{}",
            tail.map(|t| format!("/{t}")).unwrap_or_default()
        );
        d.stage = format!("{DISTRICTS_DIR}/{dir}/{}", d.stage);
        Some(district)
    }
}

fn refuse(
    out: &mut Vec<Diagnostic>,
    code: delvewright_dsl::DwCode,
    stage: &str,
    path: &str,
    m: String,
) {
    out.push(Diagnostic::error(code, stage, path, m));
}

/// The district ids the whole's raw layout graph declares, if it parses.
fn declared_districts(layout_graph: Option<&str>) -> Option<Vec<String>> {
    let v: Value = serde_json::from_str(layout_graph?).ok()?;
    Some(
        v.get("content")?
            .get("districts")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|d| d.get("id").and_then(Value::as_str).map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    )
}

/// The world document's campaign id, if it parses.
fn world_campaign_id(world: &str) -> Option<String> {
    let v: Value = serde_json::from_str(world).ok()?;
    v.get("campaign_id")?.as_str().map(str::to_string)
}

/// The document in `raw` a fragment kind unions into, when present.
fn slot_of<'a>(raw: &'a mut RawCampaign, file: &str) -> Option<&'a mut String> {
    match file {
        "npcs.json" => Some(&mut raw.npcs),
        "quests.json" => Some(&mut raw.quests),
        "dialogue.json" => Some(&mut raw.dialogue),
        "layout-graph.json" => raw.layout_graph.as_mut(),
        "detail-plan.json" => raw.detail_plan.as_mut(),
        "design.json" => raw.design.as_mut(),
        _ => None,
    }
}

/// **Read every fragment under `<dir>/districts/`, judge its shape, hash its
/// bytes into `inputs`, and union its lists into `raw`.**
///
/// Nothing is unioned while any fragment carries a refusal: a union of a
/// document the engine cannot read is a campaign nobody wrote, so the refusals
/// are returned in [`Fragments::refusals`] and the parse stops on them.
///
/// # Errors
///
/// A directory or file that exists and cannot be read.
pub fn read(
    dir: &Path,
    raw: &mut RawCampaign,
    inputs: &mut BTreeMap<String, Vec<u8>>,
) -> std::io::Result<Fragments> {
    let mut out = Fragments::default();
    let root = dir.join(DISTRICTS_DIR);
    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => {
            return Err(std::io::Error::new(
                e.kind(),
                format!("{DISTRICTS_DIR}: {e}"),
            ));
        }
    };
    let mut dirs: Vec<String> = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|e| std::io::Error::new(e.kind(), format!("{DISTRICTS_DIR}: {e}")))?;
        if entry.path().is_dir()
            && let Some(name) = entry.file_name().to_str()
        {
            dirs.push(name.to_string());
        }
    }
    dirs.sort();
    out.directories.clone_from(&dirs);
    if dirs.is_empty() {
        return Ok(out);
    }
    let declared = declared_districts(raw.layout_graph.as_deref());
    let campaign_id = world_campaign_id(&raw.world);
    for x in &dirs {
        let id = format!("{DISTRICT_PREFIX}{x}");
        let mut files: Vec<String> = Vec::new();
        for entry in std::fs::read_dir(root.join(x))
            .map_err(|e| std::io::Error::new(e.kind(), format!("{DISTRICTS_DIR}/{x}: {e}")))?
        {
            let entry = entry?;
            let p = entry.path();
            if p.is_file()
                && p.extension().and_then(|e| e.to_str()) == Some("json")
                && let Some(name) = entry.file_name().to_str()
            {
                files.push(name.to_string());
            }
        }
        files.sort();
        let is_declared = declared.as_ref().is_some_and(|d| d.contains(&id));
        if !is_declared {
            let list = declared
                .as_ref()
                .map(|d| super::ticked(d))
                .unwrap_or_else(|| {
                    "none — the campaign has no readable `layout-graph.json`".to_string()
                });
            refuse(
                &mut out.refusals,
                delvewright_dsl::DW_PARTITION,
                &format!("{DISTRICTS_DIR}/{x}"),
                "",
                format!(
                    "the directory `{DISTRICTS_DIR}/{x}/` names `{id}`, which the layout graph does \
                     not declare (it declares {list}). A `{DISTRICTS_DIR}/<x>/` directory holds the \
                     fragments of a declared district: declare `{id}` in `layout-graph.json` \
                     `districts[]`, rename the directory to a declared district, or move its \
                     objects into the whole's documents and delete it."
                ),
            );
        }
        for name in files {
            let rel = format!("{DISTRICTS_DIR}/{x}/{name}");
            let bytes = std::fs::read(root.join(x).join(&name))
                .map_err(|e| std::io::Error::new(e.kind(), format!("{rel}: {e}")))?;
            inputs.insert(rel.clone(), bytes.clone());
            let label = format!("{DISTRICTS_DIR}/{x}/{}", name.trim_end_matches(".json"));
            let Some(kind) = fragment_kind(&name) else {
                refuse(
                    &mut out.refusals,
                    DW_WHOLE_OWNS,
                    &label,
                    "",
                    format!(
                        "`{rel}` is a `{stem}` document, and a district fragments only {kinds}. \
                         The whole owns every other stage document: move its content into the \
                         whole's `{name}` (or, if it is not a stage document, out of \
                         `{DISTRICTS_DIR}/`).",
                        stem = name.trim_end_matches(".json"),
                        kinds = super::FRAGMENTS
                            .iter()
                            .map(|k| format!("`{}`", k.file))
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                );
                continue;
            };
            let text = match String::from_utf8(bytes) {
                Ok(t) => t,
                Err(e) => {
                    refuse(
                        &mut out.refusals,
                        codes::SCHEMA,
                        &label,
                        "",
                        format!("`{rel}` is not UTF-8: {e}"),
                    );
                    continue;
                }
            };
            if let Some(f) = judge(
                &rel,
                &label,
                &id,
                kind,
                &text,
                campaign_id.as_deref(),
                &mut out.refusals,
            ) {
                out.read.push(f);
            }
        }
    }
    if !out.refusals.is_empty() {
        return Ok(out);
    }
    union(raw, &mut out);
    Ok(out)
}

/// Judge one fragment's envelope and keys; return it read, or `None` with its
/// refusals pushed.
fn judge(
    rel: &str,
    label: &str,
    district: &str,
    kind: &'static FragmentKind,
    text: &str,
    campaign_id: Option<&str>,
    refusals: &mut Vec<Diagnostic>,
) -> Option<Fragment> {
    let before = refusals.len();
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            refuse(
                refusals,
                codes::SCHEMA,
                label,
                "",
                format!(
                    "`{rel}` does not parse as JSON: {e}. A district fragment is a stage \
                     document's envelope — `dsl_version`, `campaign_id`, `stage`, `content` — \
                     whose `content` carries only the lists the district may write."
                ),
            );
            return None;
        }
    };
    let Some(obj) = v.as_object() else {
        refuse(
            refusals,
            codes::SCHEMA,
            label,
            "",
            format!("`{rel}` is not a JSON object, so it is not a stage document's envelope."),
        );
        return None;
    };
    for key in obj.keys() {
        if !["dsl_version", "campaign_id", "stage", "content"].contains(&key.as_str()) {
            refuse(
                refusals,
                codes::SCHEMA,
                label,
                &format!("/{key}"),
                format!(
                    "`{rel}` carries `{key}`, which is not a key of a stage document's \
                     envelope (`dsl_version`, `campaign_id`, `stage`, `content`)."
                ),
            );
        }
    }
    let stage = obj.get("stage").and_then(Value::as_str).unwrap_or("");
    if stage != kind.stage.name() {
        refuse(
            refusals,
            delvewright_dsl::STAGE_MISMATCH,
            label,
            "/stage",
            format!(
                "`stage` is `{stage}` but `{rel}` is a fragment of the `{want}` document — set \
                 `stage` to `{want}`: a fragment names the stage it fragments.",
                want = kind.stage.name()
            ),
        );
    }
    let version = obj.get("dsl_version").and_then(Value::as_str).unwrap_or("");
    if version != delvewright_dsl::DSL_VERSION {
        refuse(
            refusals,
            codes::DSL_VERSION,
            label,
            "/dsl_version",
            format!(
                "dsl_version `{version}` is not the one this engine accepts — set it to `{}` \
                 (`delvec fmt` writes it). A fragment is read against the same surface as the \
                 document it fragments.",
                delvewright_dsl::DSL_VERSION
            ),
        );
    }
    let id = obj.get("campaign_id").and_then(Value::as_str).unwrap_or("");
    if let Some(want) = campaign_id
        && id != want
    {
        refuse(
            refusals,
            delvewright_dsl::CAMPAIGN_ID_MISMATCH,
            label,
            "/campaign_id",
            format!(
                "campaign_id `{id}` differs from `{want}` (the world stage's id) — a fragment \
                 belongs to the campaign whose directory holds it."
            ),
        );
    }
    let Some(content) = obj.get("content").and_then(Value::as_object) else {
        refuse(
            refusals,
            codes::SCHEMA,
            label,
            "/content",
            format!("`{rel}` carries no `content` object."),
        );
        return None;
    };
    let mut items = Vec::new();
    for (key, value) in content {
        if !kind.lists.contains(&key.as_str()) {
            refuse(
                refusals,
                DW_WHOLE_OWNS,
                label,
                &format!("/content/{key}"),
                format!(
                    "`{rel}` carries `{key}`, which the whole owns: a district's `{file}` may \
                     carry only {lists}, and `{key}` is written in the whole's `{file}`. Move \
                     the field there (the planner's document), or ask the planner to.",
                    file = kind.file,
                    lists = kind
                        .lists
                        .iter()
                        .map(|l| format!("`{l}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            );
            continue;
        }
        let Some(list) = value.as_array() else {
            refuse(
                refusals,
                codes::SCHEMA,
                label,
                &format!("/content/{key}"),
                format!("`{key}` in `{rel}` is not a list."),
            );
            continue;
        };
        for (i, item) in list.iter().enumerate() {
            items.push(Item {
                list: key.clone(),
                local: i,
                value: item.clone(),
            });
        }
    }
    // The fragment's lists read against their stage's own schema, so a
    // malformed object is refused naming the fragment that wrote it rather
    // than as a position in a union nobody wrote.
    if refusals.len() == before {
        let mut probe = serde_json::Map::new();
        for l in kind.lists {
            probe.insert(
                (*l).to_string(),
                content.get(*l).cloned().unwrap_or(Value::Array(Vec::new())),
            );
        }
        let probe = Value::Object(probe);
        let shape: Result<(), String> = match kind.file {
            "npcs.json" => serde_json::from_value::<delvewright_dsl::NpcsContent>(probe)
                .map(drop)
                .map_err(|e| e.to_string()),
            "quests.json" => serde_json::from_value::<delvewright_dsl::QuestsContent>(probe)
                .map(drop)
                .map_err(|e| e.to_string()),
            "dialogue.json" => serde_json::from_value::<delvewright_dsl::DialogueContent>(probe)
                .map(drop)
                .map_err(|e| e.to_string()),
            "detail-plan.json" => {
                serde_json::from_value::<delvewright_dsl::detailplan::DetailPlanContent>(probe)
                    .map(drop)
                    .map_err(|e| e.to_string())
            }
            "design.json" => serde_json::from_value::<delvewright_dsl::DesignContent>(probe)
                .map(drop)
                .map_err(|e| e.to_string()),
            _ => probe["nodes"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|n| {
                    n.get("stations")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                })
                .try_for_each(|st| {
                    serde_json::from_value::<delvewright_dsl::Station>(st.clone())
                        .map(drop)
                        .map_err(|e| e.to_string())
                }),
        };
        if let Err(e) = shape {
            refuse(
                refusals,
                codes::SCHEMA,
                label,
                "/content",
                format!(
                    "`{rel}` does not conform to the `{stage}` schema: {e}. A district fragment \
                     is the same schema as its stage — run `delvec schema --stage {stage}` for \
                     the shape of every list it may carry.",
                    stage = kind.stage.name()
                ),
            );
        }
    }
    // The finer shapes: what one object of a fragment may say.
    for it in &items {
        match kind.file {
            "layout-graph.json" => {
                let Some(node) = it.value.as_object() else {
                    refuse(
                        refusals,
                        codes::SCHEMA,
                        label,
                        &format!("/content/nodes/{}", it.local),
                        format!("node {} of `{rel}` is not an object.", it.local),
                    );
                    continue;
                };
                for key in node.keys() {
                    if key != "id" && key != "stations" {
                        refuse(
                            refusals,
                            DW_WHOLE_OWNS,
                            label,
                            &format!("/content/nodes/{}/{key}", it.local),
                            format!(
                                "node {} of `{rel}` carries `{key}`, which the whole owns: a \
                                 district's layout-graph fragment names a place by `id` and \
                                 declares its `stations[]`, and every other field of a place \
                                 is written in the whole's `layout-graph.json`.",
                                it.local
                            ),
                        );
                    }
                }
                if node.get("id").and_then(Value::as_str).is_none() {
                    refuse(
                        refusals,
                        codes::SCHEMA,
                        label,
                        &format!("/content/nodes/{}/id", it.local),
                        format!("node {} of `{rel}` names no place (`id`).", it.local),
                    );
                }
            }
            "design.json" => {
                let name = it.value.get("name").and_then(Value::as_str).unwrap_or("");
                if !name.starts_with(&format!("{}/", delvewright_dsl::design::PLACES_DIR)) {
                    refuse(
                        refusals,
                        DW_WHOLE_OWNS,
                        label,
                        &format!("/content/references/{}/name", it.local),
                        format!(
                            "row {} of `{rel}` names `{name}`, an image of the map or of a \
                             scene: those rows are the design gate's, and the whole writes them \
                             in `design.json`. A district's design fragment records the \
                             per-place images under `design/{places}/<place stem>/`.",
                            it.local,
                            places = delvewright_dsl::design::PLACES_DIR,
                        ),
                    );
                }
            }
            _ => {}
        }
    }
    if refusals.len() > before {
        return None;
    }
    Some(Fragment {
        district: district.to_string(),
        file: kind.file,
        items,
    })
}

/// The empty envelope a fragment unions into when the whole has no such
/// optional document.
fn empty_whole(kind: &FragmentKind, campaign_id: &str) -> Value {
    let mut content = serde_json::Map::new();
    for l in kind.lists {
        content.insert((*l).to_string(), Value::Array(Vec::new()));
    }
    serde_json::json!({
        "dsl_version": delvewright_dsl::DSL_VERSION,
        "campaign_id": campaign_id,
        "stage": kind.stage.name(),
        "content": content,
    })
}

/// Union every fragment into its stage document, recording where each item
/// landed. A whole document that does not parse is left as it is: its own
/// `DW0100` is the finding, and a union over it would hide which file is wrong.
fn union(raw: &mut RawCampaign, out: &mut Fragments) {
    let campaign_id = world_campaign_id(&raw.world).unwrap_or_default();
    let kinds: BTreeSet<&'static str> = out.read.iter().map(|f| f.file).collect();
    for file in kinds {
        let kind = fragment_kind(file).expect("a read fragment has a kind");
        // An optional document the whole has not written yet is created empty
        // for the fragments to union into. A graph is never created: with no
        // graph no district is declared, and `DW1003` refused every directory.
        match file {
            "detail-plan.json" if raw.detail_plan.is_none() => {
                raw.detail_plan = Some(empty_whole(kind, &campaign_id).to_string());
            }
            "design.json" if raw.design.is_none() => {
                raw.design = Some(empty_whole(kind, &campaign_id).to_string());
            }
            _ => {}
        }
        let Some(slot) = slot_of(raw, file) else {
            continue;
        };
        let Ok(mut whole) = serde_json::from_str::<Value>(slot) else {
            continue;
        };
        if file == "layout-graph.json" {
            union_stations(&mut whole, out);
        } else {
            for list in kind.lists {
                let Some(content) = whole.get_mut("content").and_then(Value::as_object_mut) else {
                    break;
                };
                let arr = content
                    .entry((*list).to_string())
                    .or_insert_with(|| Value::Array(Vec::new()));
                let Some(arr) = arr.as_array_mut() else {
                    continue;
                };
                for f in out.read.iter().filter(|f| f.file == file) {
                    let start = arr.len();
                    let mut len = 0usize;
                    for it in f.items.iter().filter(|i| i.list == *list) {
                        arr.push(it.value.clone());
                        len += 1;
                    }
                    if len > 0 {
                        out.spans
                            .entry((kind.stage.name().to_string(), (*list).to_string()))
                            .or_default()
                            .push(Span {
                                district: f.district.clone(),
                                start,
                                len,
                            });
                    }
                }
            }
        }
        *slot = whole.to_string();
    }
}

/// Union the stations a layout-graph fragment declares onto the whole's
/// nodes. A node the whole's graph does not declare is deferred to the located
/// rule, which names it.
fn union_stations(whole: &mut Value, out: &mut Fragments) {
    let Some(nodes) = whole
        .get_mut("content")
        .and_then(|c| c.get_mut("nodes"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    out.node_order = nodes
        .iter()
        .map(|n| {
            n.get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        })
        .collect();
    for n in nodes.iter() {
        let id = n
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let own = n
            .get("stations")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        out.stations.entry(id).or_insert((own, Vec::new()));
    }
    let fragments: Vec<Fragment> = out
        .read
        .iter()
        .filter(|f| f.file == "layout-graph.json")
        .cloned()
        .collect();
    for f in &fragments {
        for it in &f.items {
            let id = it.value.get("id").and_then(Value::as_str).unwrap_or("");
            let Some(pos) = out.node_order.iter().position(|n| n == id) else {
                out.deferred.push(Diagnostic::error(
                    super::DW_OUTSIDE_DISTRICT,
                    f.stage_label(),
                    format!("/content/nodes/{}/id", it.local),
                    format!(
                        "`{path}` declares stations on `{id}`, which the layout graph does not \
                         declare. A district declares stations only on its own places; the \
                         places of `{district}` are named in the whole's `layout-graph.json` \
                         `districts[]`.",
                        path = f.path(),
                        district = f.district,
                    ),
                ));
                continue;
            };
            let added: Vec<Value> = it
                .value
                .get("stations")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if added.is_empty() {
                continue;
            }
            let node = &mut nodes[pos];
            let Some(obj) = node.as_object_mut() else {
                continue;
            };
            let arr = obj
                .entry("stations".to_string())
                .or_insert_with(|| Value::Array(Vec::new()));
            let Some(arr) = arr.as_array_mut() else {
                continue;
            };
            let start = arr.len();
            arr.extend(added.iter().cloned());
            out.stations
                .get_mut(id)
                .expect("every node is entered above")
                .1
                .push(StationSpan {
                    district: f.district.clone(),
                    local_node: it.local,
                    start,
                    len: added.len(),
                });
        }
    }
}
