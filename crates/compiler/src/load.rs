//! Load a campaign directory (`world.json`, `npcs.json`, …) into the DSL's
//! [`RawCampaign`], keeping the raw bytes for input hashing (manifest).

use std::collections::BTreeMap;
use std::path::Path;

use delvewright_dsl::RawCampaign;

/// The six stage filenames a campaign directory must contain.
pub const STAGE_FILES: [&str; 6] = [
    "world.json",
    "npcs.json",
    "classes.json",
    "quest-plan.json",
    "quests.json",
    "dialogue.json",
];

/// The optional stage-7 edit-script filename (spec-0017). Not in
/// [`STAGE_FILES`]: a campaign without one builds byte-identically to a
/// pre-stage-7 campaign, so its absence is never an error.
pub const WORLD_EDITS_FILE: &str = "world-edits.json";

/// The optional map-pipeline documents (spec-0049), on the same terms as the
/// edit script: absent is never an error, and present is hashed into the
/// manifest inputs like any other stage document.
pub const GEOMETRY_BRIEF_FILE: &str = "geometry-brief.json";

/// The layout graph's filename (spec-0049 §3) — see [`GEOMETRY_BRIEF_FILE`].
pub const LAYOUT_GRAPH_FILE: &str = "layout-graph.json";

/// The site plan's filename (spec-0049 §4) — see [`GEOMETRY_BRIEF_FILE`].
///
/// The loader finds it like any other stage document, and the ordering it sits
/// under is enforced where it can name what is missing: a plan with no layout
/// graph or no geometry brief beside it is `DW0824`, by name, at validation.
/// Refusing to LOAD the file would have said the same thing in a message about
/// a path, to an author who had just written it.
pub const SITE_PLAN_FILE: &str = "site-plan.json";

/// The detail plan's filename (spec-0050 §1) — see [`GEOMETRY_BRIEF_FILE`].
pub const DETAIL_PLAN_FILE: &str = "detail-plan.json";

/// **The walk record's filename** (spec-0049 §5.4, gated by spec-0050 §2).
///
/// Not a stage document: it carries no `dsl_version`, no `campaign_id` and no
/// `stage`, because it is not authored against a schema version — it is the
/// record of a human walking one particular derived blockout, and its form was
/// fixed by the spec that produced the blockout rather than by the DSL.
///
/// It is deliberately **not** hashed into the manifest inputs. It reaches no
/// emitted byte: it gates whether detail work may proceed at all, and a build
/// whose record was merely re-recorded must stay byte-identical, or double-build
/// determinism would become a property of when somebody last walked the map.
pub const WALK_RECORD_FILE: &str = "walk-record.json";

/// A loaded campaign directory: the parsed-ready [`RawCampaign`] plus the exact
/// raw file contents (by filename) for deterministic input hashing.
pub struct LoadedCampaign {
    /// The six raw stage documents, ready for `parse_campaign`.
    pub raw: RawCampaign,
    /// Filename → exact file bytes, for `manifest.json` input hashes.
    pub inputs: BTreeMap<String, Vec<u8>>,
    /// i18n l10n sidecars found under `l10n/`: language code (filename stem) →
    /// raw sidecar bytes. Empty when the campaign ships no `l10n/` directory.
    pub l10n: BTreeMap<String, Vec<u8>>,
    /// `walk-record.json`, verbatim, when the campaign directory ships one —
    /// see [`WALK_RECORD_FILE`] for why it travels beside the stage documents
    /// rather than among them.
    pub walk_record: Option<String>,
}

/// Read all six stage files from `dir` plus any `l10n/*.json` sidecars. Fails if a
/// stage file is missing/unreadable; a missing `l10n/` directory is not an error
/// (English-only campaign).
pub fn load_campaign_dir(dir: &Path) -> std::io::Result<LoadedCampaign> {
    let mut inputs = BTreeMap::new();
    let mut read = |name: &str| -> std::io::Result<String> {
        let bytes = std::fs::read(dir.join(name))?;
        let s = String::from_utf8(bytes.clone()).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{name}: {e}"))
        })?;
        inputs.insert(name.to_string(), bytes);
        Ok(s)
    };
    let world = read("world.json")?;
    let npcs = read("npcs.json")?;
    let classes = read("classes.json")?;
    let quest_plan = read("quest-plan.json")?;
    let quests = read("quests.json")?;
    let dialogue = read("dialogue.json")?;
    // The optional stage-7 edit script (spec-0017): absent = no edit stage
    // (byte-identical build); present = loaded, parsed, validated and hashed
    // into the manifest inputs like any other stage document.
    let world_edits = if dir.join(WORLD_EDITS_FILE).is_file() {
        Some(read(WORLD_EDITS_FILE)?)
    } else {
        None
    };
    let geometry_brief = if dir.join(GEOMETRY_BRIEF_FILE).is_file() {
        Some(read(GEOMETRY_BRIEF_FILE)?)
    } else {
        None
    };
    let layout_graph = if dir.join(LAYOUT_GRAPH_FILE).is_file() {
        Some(read(LAYOUT_GRAPH_FILE)?)
    } else {
        None
    };
    let site_plan = if dir.join(SITE_PLAN_FILE).is_file() {
        Some(read(SITE_PLAN_FILE)?)
    } else {
        None
    };
    let detail_plan = if dir.join(DETAIL_PLAN_FILE).is_file() {
        Some(read(DETAIL_PLAN_FILE)?)
    } else {
        None
    };
    // Read outside the `read` closure on purpose: that closure records a
    // filename into `inputs`, and the walk record is not a build input — see
    // [`WALK_RECORD_FILE`].
    let walk_record = match std::fs::read_to_string(dir.join(WALK_RECORD_FILE)) {
        Ok(s) => Some(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    let l10n = load_l10n_dir(&dir.join("l10n"))?;
    // i18n v2 (spec-0029): every sidecar is a build input of **every** build, not
    // just of a `--lang` bake — the delve now ships each declared language's lang
    // file in its resource pack, so the sidecar's bytes are as much a build input
    // as a stage document, and the manifest hashes them like one.
    for (code, bytes) in &l10n {
        inputs.insert(format!("l10n/{code}.json"), bytes.clone());
    }
    Ok(LoadedCampaign {
        raw: RawCampaign {
            world,
            npcs,
            classes,
            quest_plan,
            quests,
            dialogue,
            world_edits,
            geometry_brief,
            layout_graph,
            site_plan,
            detail_plan,
        },
        walk_record,
        inputs,
        l10n,
    })
}

/// Read every `<code>.json` sidecar in an `l10n/` directory → `code` (filename
/// stem) → raw bytes. Returns an empty map if the directory does not exist.
/// Iteration is sorted for determinism (ADR-0006).
fn load_l10n_dir(l10n_dir: &Path) -> std::io::Result<BTreeMap<String, Vec<u8>>> {
    let mut out = BTreeMap::new();
    if !l10n_dir.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(l10n_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(code) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        out.insert(code.to_string(), std::fs::read(&path)?);
    }
    Ok(out)
}
