//! Load a campaign directory (`world.json`, `npcs.json`, …) into the DSL's
//! [`RawCampaign`], keeping the raw bytes for input hashing (manifest).

use std::collections::BTreeMap;
use std::path::Path;

use delvewright_dsl::{Diagnostic, DwCode, RawCampaign};

/// `DW0874`: a campaign directory is present and does not hold all six stage
/// documents.
///
/// **The state this names is the one the authoring skill tells an author to be
/// in.** A campaign is written a document at a time, and until the sixth is
/// written the directory is incomplete by construction. Every one of the four
/// verbs that reads a campaign directory used to answer that with
/// `internal error: cannot read campaign dir: npcs.json`, exit 10, and no code
/// at all — the phrasing this compiler reserves for its own bugs, printed at the
/// first thing it ever says to a new author, about the thing the page had just
/// told them to do.
///
/// Being uncoded was the load-bearing half. Every other authoring mistake here
/// is a `DW` code with a documented row, an exit of 1, and a sentence saying what
/// to write; this one had none of the three, so nothing about it could be looked
/// up, asserted by a test, or told apart from a crash.
///
/// Validation tier (exit 1), because that is what it is: the campaign is refused,
/// the compiler is fine. Raised through [`delvewright_dsl::Fenced::structural`],
/// which is the fence for a finding that exists **before a campaign has parsed** —
/// there is no declared `dsl_version` to grandfather against, which is also why
/// the code binds every version.
pub const DW_STAGE_DOCUMENT_MISSING: DwCode = DwCode::every_version("DW0874");

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

/// **Every document a campaign directory may carry and is never refused for
/// omitting**, in the order a diagnostic should name them.
///
/// Written down here, beside [`STAGE_FILES`], because the two lists are the
/// halves of one fact: what a campaign directory must have, and what it may.
/// A refusal that names only the first half tells an author which six documents
/// are required and leaves them unable to tell whether the seventh they have not
/// written is the next thing they owe.
pub const OPTIONAL_FILES: [&str; 6] = [
    WORLD_EDITS_FILE,
    GEOMETRY_BRIEF_FILE,
    LAYOUT_GRAPH_FILE,
    SITE_PLAN_FILE,
    DETAIL_PLAN_FILE,
    WALK_RECORD_FILE,
];

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

/// **Which of the six stage documents this directory does not have** — the
/// authoring state, told apart from the crash.
///
/// A campaign directory missing one of the six is not a compiler fault and never
/// was: it is what a campaign looks like while it is being written, and it is the
/// state the authoring skill tells an author to be in ("stub the later stages").
/// Reported as `internal error` with no code, it was the first thing this
/// compiler ever said to a new author, and it said the tool was broken.
///
/// Absence is probed by **opening**, never by `is_file()`. A directory standing
/// where `npcs.json` belongs answers `false` to `is_file()`, so a probe built on
/// it would call an unreadable document absent and send the author to write one
/// they already have — the shape [`optional`] carries the same rule for.
/// `NotFound` is absent; every other error means the document is there and
/// something else is wrong, which is [`load_campaign_dir`]'s own error to raise.
///
/// Returns the missing names in [`STAGE_FILES`] order — document order, so the
/// list reads as a position in the authoring sequence rather than as a set.
///
/// **Empty when `dir` is not a directory**, and that is a correctness rule rather
/// than a guard. A path that does not exist has six absent stage documents by
/// arithmetic, and answering `world.json, npcs.json, …` for a mistyped path would
/// be a true sentence about a campaign directory that is not there — a worse
/// message than the one it replaced, and a refusal whose remedy (write six
/// documents) does nothing. Absence OF the directory is [`load_campaign_dir`]'s
/// own finding, which it already names by path.
#[must_use]
pub fn missing_stage_documents(dir: &Path) -> Vec<&'static str> {
    if !matches!(std::fs::metadata(dir), Ok(m) if m.is_dir()) {
        return Vec::new();
    }
    STAGE_FILES
        .iter()
        .copied()
        .filter(|name| {
            matches!(
                std::fs::File::open(dir.join(name)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound
            )
        })
        .collect()
}

/// Render a list of filenames as `` `a`, `b` and `c` ``.
fn and_list(names: &[&str]) -> String {
    let quoted: Vec<String> = names.iter().map(|n| format!("`{n}`")).collect();
    match quoted.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// The [`DW_STAGE_DOCUMENT_MISSING`] refusal `dir` has earned, or `None` when it
/// holds all six stage documents.
///
/// **One writer for the sentence.** Four CLI verbs read a campaign directory and
/// all four printed the same uncoded internal error beside their own copy of the
/// handling; a rule that lives in one call site's error arm is a rule the next
/// three callers cannot reuse.
///
/// It names **every** missing document, not the first. The loader reads in
/// document order and stops at the first absence, so an author starting from
/// `world.json` alone learned the remaining five filenames by running `validate`
/// five more times. Five runs to be told a fixed list is not an affordance.
///
/// Both lists are rendered from [`STAGE_FILES`] and [`OPTIONAL_FILES`] rather
/// than written out here: a hand-copied enumeration is how a seventh document
/// would arrive without the sentence that tells authors it exists.
#[must_use]
pub fn missing_stage_documents_diagnostic(dir: &Path) -> Option<Diagnostic> {
    let missing = missing_stage_documents(dir);
    if missing.is_empty() {
        return None;
    }
    Some(Diagnostic::error(
        DW_STAGE_DOCUMENT_MISSING,
        "campaign",
        dir.display().to_string(),
        format!(
            "this campaign directory is missing {n} of the {total} stage documents every \
             campaign is made of: {missing}. That is an authoring state, not a fault — a \
             campaign is written a document at a time — but all {total} have to be ON DISK \
             before any verb can read the campaign, so the way to work through them in order \
             is to STUB the ones you have not reached yet, never to leave them out. The {total} \
             are {all}. A stub is that document's envelope and nothing else: `dsl_version`, \
             `campaign_id`, `stage`, and a `content` carrying only the fields its schema \
             requires. Run `delvec schema --stage <name>` for the exact shape of each one, \
             where `<name>` is the filename without `.json` ({names}). Stubbing is not the \
             same as omitting: a stub is a campaign that has not declared anything yet, so \
             the next thing it owes comes back as an ordinary diagnostic naming it — `DW0100` \
             for whatever its schema requires and has not been given. The \
             optional documents are NOT in this list and are never required — {optional}, and \
             an `l10n/` directory — so their absence is not what stopped this run.",
            n = missing.len(),
            total = STAGE_FILES.len(),
            missing = and_list(&missing),
            all = and_list(&STAGE_FILES),
            names = STAGE_FILES
                .iter()
                .map(|f| format!("`{}`", f.trim_end_matches(".json")))
                .collect::<Vec<_>>()
                .join(", "),
            optional = and_list(&OPTIONAL_FILES),
        ),
    ))
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
