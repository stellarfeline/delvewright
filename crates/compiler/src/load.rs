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

/// Attach the campaign-relative name of the document being read to a filesystem
/// error, keeping its [`std::io::ErrorKind`] intact.
///
/// The kind is preserved deliberately. **Absent** and **unreadable** are
/// different findings for an author — one is a document not written yet, the
/// other is one that cannot be opened — so answering *which file* by flattening
/// both into a string would trade one missing half of the message for another.
fn named(what: impl std::fmt::Display, e: std::io::Error) -> std::io::Error {
    std::io::Error::new(e.kind(), format!("{what}: {e}"))
}

/// An optional stage document is absent only when it is **not there**.
///
/// Anything else — a directory standing in its place, a file that cannot be
/// opened — is a finding. Probing with `is_file()` and reading only on success
/// answers *absent* for every one of those cases, and the build then ships a
/// campaign missing a stage document **byte-identically** to one that never
/// declared it, with nothing downstream able to tell the two apart. That is a
/// silent wrong build, which is worse than a refusal.
///
/// [`WALK_RECORD_FILE`] has always been read this way; its five siblings carried
/// the weaker probe, so the rule already lived in this file and reached one of
/// the six documents it should govern.
fn optional(r: std::io::Result<String>) -> std::io::Result<Option<String>> {
    match r {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Read all six stage files from `dir` plus any `l10n/*.json` sidecars. Fails if a
/// stage file is missing/unreadable; a missing `l10n/` directory is not an error
/// (English-only campaign).
///
/// **Every failure names the document it is about.** A campaign directory that
/// is present and missing one of six documents is an ordinary authoring state,
/// and the author cannot act on a message that says only that something under
/// the directory could not be read.
pub fn load_campaign_dir(dir: &Path) -> std::io::Result<LoadedCampaign> {
    // Establish the directory itself before reading a single document. Naming
    // the document is exactly what makes this necessary: without it, an absent
    // campaign directory fails on `world.json` and reports a missing DOCUMENT,
    // sending the author after one file inside a directory that is not there.
    // The two are different findings and the message has to be able to say
    // which one it is.
    match std::fs::metadata(dir) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                format!("{}: not a campaign directory", dir.display()),
            ));
        }
        Err(e) => return Err(named(dir.display(), e)),
    }
    let mut inputs = BTreeMap::new();
    let mut read = |name: &str| -> std::io::Result<String> {
        let bytes = std::fs::read(dir.join(name)).map_err(|e| named(name, e))?;
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
    let world_edits = optional(read(WORLD_EDITS_FILE))?;
    let geometry_brief = optional(read(GEOMETRY_BRIEF_FILE))?;
    let layout_graph = optional(read(LAYOUT_GRAPH_FILE))?;
    let site_plan = optional(read(SITE_PLAN_FILE))?;
    let detail_plan = optional(read(DETAIL_PLAN_FILE))?;
    // Read outside the `read` closure on purpose: that closure records a
    // filename into `inputs`, and the walk record is not a build input — see
    // [`WALK_RECORD_FILE`].
    let walk_record = match std::fs::read_to_string(dir.join(WALK_RECORD_FILE)) {
        Ok(s) => Some(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(named(WALK_RECORD_FILE, e)),
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
    for entry in std::fs::read_dir(l10n_dir).map_err(|e| named("l10n", e))? {
        let path = entry.map_err(|e| named("l10n", e))?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(code) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let bytes = std::fs::read(&path).map_err(|e| named(format!("l10n/{code}.json"), e))?;
        out.insert(code.to_string(), bytes);
    }
    Ok(out)
}
