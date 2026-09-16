//! **The approved hour is the built hour** (`DW0890`, spec-0061).
//!
//! A delve's look is approved by a human looking at a picture. Nothing in this
//! toolchain reads a picture, so what is held to the world is the token the
//! creator wrote beside it at the moment of approving: `design.json`'s rows,
//! each naming an image under `design/` and the sky it was drawn under.
//!
//! This module compares three things that must agree and, until it existed,
//! never met:
//!
//! 1. the **rows** of `design.json` — the skies the approved pictures show;
//! 2. the **files** under `design/concept/` and `design/reference/` — the
//!    pictures themselves;
//! 3. the **world's reachable skies** — [`crate::compiler::light::reachable_time_weather`],
//!    the same scan `DW0210` reads to find the darkest hour and `DW0496` reads
//!    to ask whether the sun always burns.
//!
//! # Why the reachable-state scan and not a private one
//!
//! Because a third reading of the campaign's clock is a third chance for the
//! three to disagree. `reachable_time_weather` already walks the declared
//! initial state plus every `set-time` / `set-weather` target through every
//! effect root at every depth, quest and dialogue both. A private scan here
//! would be the shallow, root-listing walk that scan itself had to be repaired
//! of.
//!
//! # Why validation and not the build
//!
//! `DW0855`'s own note states the rule: refused where it is a fact about the
//! documents, because nothing has to be placed to know it. The rows are a
//! document, the reachable skies are computed from documents, and the images
//! are names in a directory. The drill this rule comes from paid a ~1 min build
//! and 82 minutes of a 109-minute render to learn by eye that its night delve
//! was built at noon; the same fact is available in under a second, before a
//! single piece is seated.
//!
//! # What is deliberately not here
//!
//! No image bytes are opened, and no model reads a picture. A classifier would
//! be a measurement with an unstated error rate standing where a refusal
//! belongs — the shape spec-0028 §3 already forbids for `refscore`. Whether a
//! row's tokens are *true of the picture* is a human reading, made once, at
//! approval.

use std::collections::BTreeSet;
use std::path::Path;

use delvewright_dsl::{
    CONCEPT_DIR, Campaign, Diagnostic, DwCode, ExitTier, REFERENCE_DIR, REFERENCE_DIRS, Reference,
    WorldTime, WorldWeather, is_image_extension,
};

use crate::compiler::view::camera::Answers;

use crate::compiler::light::reachable_time_weather;

/// `DW0890`: **the approved design and the built world do not agree about the
/// sky**, in one of three ways — a sky nobody approved a picture of, a row
/// naming an image that is not there, or an approved image nobody recorded.
///
/// One code, because the three are one fact seen from three sides: the record
/// and the world are not the same delve. Splitting them would give a creator
/// three numbers to look up for one repair, and the repair is always the same
/// pair of moves — change the world, or change the record.
///
/// Validation tier (exit 1). Its `ExitTier::Build` declaration is the ordinary
/// one every validation diagnostic carries and says what it means: if this rule
/// refuses with a build under way, it stops the build.
pub const DW_DESIGN_SKY: DwCode = DwCode::new("DW0890", ExitTier::Build);

/// `DW0900`: **an approved picture has no showcase camera** — a row of
/// `design.json` that no camera of `design/cameras.json` answers (spec-0070).
///
/// One code for one fact, separate from [`DW_DESIGN_SKY`] because its repair is
/// unrelated: a creator writes a camera against the last built tree, where
/// `DW0890`'s repair changes the hour or the record and `DW0721`'s fixes the
/// record's own rules. A creator looks a code up to find its repair.
///
/// Build tier (exit 3), and `delvec cameras` raises the same code at its own
/// exit 2 when it emits scenes — one code in two tiers, as `DW0721` already is.
/// It refuses at the build rather than at validation because every view command
/// validates first, so a validation-tier refusal would refuse `cameras
/// --preview`, the instrument that completes the record (spec-0070 §3).
pub const DW_DESIGN_ANSWERED: DwCode = DwCode::new("DW0900", ExitTier::Build);

/// One approved image found under `design/`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DesignImage {
    /// The name a row would use: `concept/shore-far` — the directory and the
    /// file's stem, with no extension.
    pub name: String,
    /// The file as it is on disk, relative to `design/`: `concept/shore-far.png`.
    pub file: String,
}

/// **The image files a campaign's `design/` directory holds**, read once by the
/// loader and carried beside the stage documents.
///
/// Empty means *there are no approved images*, which is a measured zero and the
/// ordinary state of a campaign that has not reached its design step. It never
/// means "I could not look": a `design/` that cannot be read is the loader's
/// own error, named by path.
///
/// It also carries the bytes of `design/cameras.json`, the showcase camera
/// record (`compiler::view::camera`), when the campaign has one: the build proves
/// every camera in it against the assembled world (`DW0724`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesignFiles {
    /// Every image file under `design/concept/` and `design/reference/`, sorted
    /// by `file` (ADR-0006 — the order is the record's, never the filesystem's).
    pub images: Vec<DesignImage>,
    /// `design/cameras.json`, byte for byte; `None` when it is not there.
    pub cameras: Option<Vec<u8>>,
}

impl DesignFiles {
    /// Read `<campaign>/design/` — the two directories an approved image lives
    /// in — and collect every image file in them.
    ///
    /// Only the two directories are read, and only files whose extension is in
    /// [`delvewright_dsl::IMAGE_EXTENSIONS`] are collected. `design/` also
    /// carries the re-issue sidecars `tools/refimg.py` writes and whatever
    /// prose the creator keeps beside their pictures; a file that is not an
    /// image is not this record's subject, and is neither counted nor refused.
    ///
    /// A directory that is not there yields nothing — a campaign with no
    /// approved design is not an error at this tier.
    pub fn read(campaign_dir: &Path) -> std::io::Result<DesignFiles> {
        let mut images = Vec::new();
        for dir in REFERENCE_DIRS {
            let path = campaign_dir.join("design").join(dir);
            let entries = match std::fs::read_dir(&path) {
                Ok(e) => e,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    return Err(std::io::Error::new(e.kind(), format!("design/{dir}: {e}")));
                }
            };
            for entry in entries {
                let p = entry?.path();
                let Some(ext) = p.extension().and_then(|e| e.to_str()) else {
                    continue;
                };
                if !is_image_extension(&ext.to_ascii_lowercase()) {
                    continue;
                }
                let (Some(stem), Some(file)) = (
                    p.file_stem().and_then(|s| s.to_str()),
                    p.file_name().and_then(|s| s.to_str()),
                ) else {
                    continue;
                };
                images.push(DesignImage {
                    name: format!("{dir}/{stem}"),
                    file: format!("{dir}/{file}"),
                });
            }
        }
        images.sort();
        let record = campaign_dir.join(crate::compiler::view::camera::CAMERAS_FILE);
        let cameras = match std::fs::read(&record) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                return Err(std::io::Error::new(
                    e.kind(),
                    format!("{}: {e}", crate::compiler::view::camera::CAMERAS_FILE),
                ));
            }
        };
        Ok(DesignFiles { images, cameras })
    }

    /// Every file whose stem is `name`, in `file` order.
    fn resolve(&self, name: &str) -> Vec<&str> {
        self.images
            .iter()
            .filter(|i| i.name == name)
            .map(|i| i.file.as_str())
            .collect()
    }

    /// How many images are under `dir`.
    fn count_in(&self, dir: &str) -> usize {
        self.images
            .iter()
            .filter(|i| i.name.starts_with(&format!("{dir}/")))
            .count()
    }
}

/// What this module examined, printed on every run whether or not it refuses.
///
/// A campaign with no `design/` and no `design.json` prints zeroes and does not
/// refuse: that is a measured zero of an optional surface, and staging — where
/// a zero of this class is a red — is the tier that judges it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesignBinding {
    /// Rows in `design.json`.
    pub references: usize,
    /// Image files found under `design/`.
    pub image_files: usize,
    /// Of those, the ones under `concept/`.
    pub concept_files: usize,
    /// Of those, the ones under `reference/`.
    pub reference_files: usize,
    /// The distinct `(time, weather)` pairs the rows state, with how many rows
    /// state each, in declaration order of the two enums.
    pub skies_stated: Vec<(WorldTime, WorldWeather, usize)>,
    /// Every time state the world can reach.
    pub world_times: Vec<WorldTime>,
    /// Every weather state the world can reach.
    pub world_weathers: Vec<WorldWeather>,
    /// The showcase camera record beside the rows (spec-0070): absent, refused
    /// by its own reader, or read and counted.
    pub cameras: CameraRecord,
}

/// **What the showcase camera record was at this run**, and what it answered.
///
/// Three states rather than a count, because a zero has to say which fact it
/// is: a campaign between its design step and its first build has no record at
/// all, and a record its reader refuses is a different thing again — one the
/// build stops for under `DW0721`, and one this gate never turns into a number.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CameraRecord {
    /// No `design/cameras.json`. The ordinary state of a campaign that has not
    /// placed a camera yet.
    #[default]
    Absent,
    /// Present, and refused by its one reader (`compiler::view::camera`). The
    /// message is that reader's; this gate counts nothing from it.
    Unreadable(String),
    /// Present and read: what it answers of the rows.
    Read(Answers),
}

impl CameraRecord {
    /// The cameras the record states — `0` for a record that is not there and
    /// for one nothing could read.
    pub fn cameras(&self) -> usize {
        match self {
            CameraRecord::Read(a) => a.cameras,
            _ => 0,
        }
    }

    /// Rows with at least one camera.
    pub fn answered(&self) -> usize {
        match self {
            CameraRecord::Read(a) => a.answered.len(),
            _ => 0,
        }
    }

    /// Rows no camera answers, in the design's order — every row when there is
    /// no readable record.
    pub fn unanswered<'a>(&'a self, rows: &'a [Reference]) -> Vec<String> {
        match self {
            CameraRecord::Read(a) => a.unanswered.clone(),
            _ => rows.iter().map(|r| r.name.clone()).collect(),
        }
    }
}

/// Render a set of tokens as `{a, b}` — `{}` when it is empty.
fn set_line<T: Copy>(items: &[T], token: impl Fn(T) -> &'static str) -> String {
    format!(
        "{{{}}}",
        items
            .iter()
            .map(|&t| token(t))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

impl DesignBinding {
    /// The one-line binding statement a run prints, zeroes included.
    pub fn line(&self) -> String {
        let skies = if self.skies_stated.is_empty() {
            "none".to_string()
        } else {
            self.skies_stated
                .iter()
                .map(|(t, w, n)| format!("{}+{} x{n}", t.keyword(), w.keyword()))
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "design record: {} reference(s) recorded over {} image file(s) under `design/` \
             ({CONCEPT_DIR}/ {}, {REFERENCE_DIR}/ {}); skies stated: {skies}; world reaches times \
             {} weathers {} (DW0890); {}",
            self.references,
            self.image_files,
            self.concept_files,
            self.reference_files,
            set_line(&self.world_times, WorldTime::keyword),
            set_line(&self.world_weathers, WorldWeather::keyword),
            self.cameras_line(),
        )
    }

    /// The showcase half of the binding line (spec-0070 §5), zeroes included:
    /// how many cameras the record states and how many approved images they
    /// answer. Printed on every run, whether or not there is a record.
    pub fn cameras_line(&self) -> String {
        let n = self.references;
        match &self.cameras {
            CameraRecord::Absent => format!(
                "showcase cameras: none (no {file}); 0 of {n} approved image(s) answered (DW0900)",
                file = crate::compiler::view::camera::CAMERAS_FILE,
            ),
            CameraRecord::Unreadable(why) => format!(
                "showcase cameras: {file} is refused by its reader, so nothing counts it \
                 (DW0721: {why}); 0 of {n} approved image(s) answered (DW0900)",
                file = crate::compiler::view::camera::CAMERAS_FILE,
            ),
            CameraRecord::Read(a) => format!(
                "showcase cameras: {} in {file} answering {} of {n} approved image(s){stray} (DW0900)",
                a.cameras,
                a.answered.len(),
                file = crate::compiler::view::camera::CAMERAS_FILE,
                stray = if a.stray.is_empty() {
                    String::new()
                } else {
                    format!(
                        "; {} camera(s) name a row that does not exist ({})",
                        a.stray.len(),
                        a.stray
                            .iter()
                            .map(|(name, answers)| format!("`{name}` -> `{answers}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                },
            ),
        }
    }
}

/// `validation/design-record.json` — what the design gate looked at, written on
/// **every** build.
///
/// Including a build with no record at all: `references: 0` is a number the
/// staging gate reads and refuses on, and an absent file is *I could not look*,
/// which is a different fact and would red the gate as format rot instead.
pub fn record(binding: &DesignBinding, findings: &Findings) -> serde_json::Value {
    serde_json::json!({
        "references": binding.references,
        "image_files": binding.image_files,
        "by_directory": {
            CONCEPT_DIR: binding.concept_files,
            REFERENCE_DIR: binding.reference_files,
        },
        "skies_stated": binding
            .skies_stated
            .iter()
            .map(|(t, w, n)| serde_json::json!({
                "time": t.keyword(),
                "weather": w.keyword(),
                "count": n,
            }))
            .collect::<Vec<_>>(),
        "world": {
            "times": binding.world_times.iter().map(|t| t.keyword()).collect::<Vec<_>>(),
            "weathers": binding.world_weathers.iter().map(|w| w.keyword()).collect::<Vec<_>>(),
        },
        "unrecorded_files": findings.unrecorded_files,
        "unresolved_rows": findings.unresolved_rows,
        "cameras": binding.cameras.cameras(),
        "answered": binding.cameras.answered(),
        "unanswered_rows": findings.unanswered_rows,
    })
}

/// The two lists `validation/design-record.json` carries beside its counts: the
/// approved images no row records, and the rows no image answers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Findings {
    /// Files under `design/` with no row, in `file` order.
    pub unrecorded_files: Vec<String>,
    /// Row names that resolve to no file, or to more than one, in row order.
    pub unresolved_rows: Vec<String>,
    /// Row names no showcase camera answers, in row order (spec-0070): every
    /// row when the campaign has no readable `design/cameras.json`, which is the
    /// case the staging gate refuses and the build cannot.
    pub unanswered_rows: Vec<String>,
}

/// Run the design gate: the binding, the refusals, and the two lists the
/// artifact carries.
///
/// `files` is what the campaign directory holds; a campaign compiled from
/// documents with no directory behind them passes [`DesignFiles::default`],
/// which is the honest reading — there are no approved image files.
pub fn check(c: &Campaign, files: &DesignFiles) -> (Vec<Diagnostic>, DesignBinding, Findings) {
    let (world_times, world_weathers) = reachable_time_weather(c);
    let rows: &[Reference] = c
        .design
        .as_ref()
        .map(|e| e.content.references.as_slice())
        .unwrap_or(&[]);

    let mut binding = DesignBinding {
        references: rows.len(),
        image_files: files.images.len(),
        concept_files: files.count_in(CONCEPT_DIR),
        reference_files: files.count_in(REFERENCE_DIR),
        skies_stated: Vec::new(),
        world_times: world_times.clone(),
        world_weathers: world_weathers.clone(),
        cameras: camera_record(files, rows),
    };
    // Counted through an ordered map so the artifact and the binding line are
    // byte-stable (ADR-0006): the key is the pair's declaration order in the
    // two enums, never the order the rows happen to be written in.
    let mut sky_counts: std::collections::BTreeMap<(usize, usize), usize> = Default::default();
    for r in rows {
        *sky_counts
            .entry((time_index(r.time), weather_index(r.weather)))
            .or_insert(0) += 1;
    }
    binding.skies_stated = sky_counts
        .into_iter()
        .map(|((t, w), n)| (TIMES[t], WEATHERS[w], n))
        .collect();

    let mut d = Vec::new();
    let mut findings = Findings {
        unanswered_rows: binding.cameras.unanswered(rows),
        ..Findings::default()
    };

    // ---- shape b: a row names an image that is not there --------------------
    for (i, r) in rows.iter().enumerate() {
        // A row whose name is not a path at all is `DW0110`'s finding, already
        // raised at the DSL tier; resolving it here would report the same
        // defect twice under a code that is about the sky.
        if !r.is_valid_name() {
            continue;
        }
        let hits = files.resolve(&r.name);
        if hits.len() == 1 {
            continue;
        }
        findings.unresolved_rows.push(r.name.clone());
        let dir = r.directory().unwrap_or(CONCEPT_DIR);
        let present = present_list(files, dir);
        let what = if hits.is_empty() {
            format!("nothing under `design/{dir}/` has that stem (files present: {present})",)
        } else {
            format!(
                "{n} files under `design/{dir}/` answer to that stem — {cands}. A resolve by name \
                 over a directory where the names are not unique yields a candidate, not a match, \
                 so this record does not say which picture was approved",
                n = hits.len(),
                cands = quoted(&hits),
            )
        };
        d.push(Diagnostic::error(
            DW_DESIGN_SKY,
            "design",
            format!("/content/references/{i}/name"),
            format!(
                "design record row {i} names `{name}` and {what}. A row is the approval of one \
                 file; with no file it approves nothing, and the sky it states is held against \
                 this world for a picture nobody can look at. Either (1) AUTHOR the approved \
                 image into `design/{dir}/` under that stem, or (2) DELETE the row from \
                 `design.json`. Do not rename the row to whichever file happens to be there — \
                 the name is which picture was approved, and changing it re-approves a different \
                 one.",
                name = r.name,
            ),
        ));
    }

    // ---- shape c: an approved image nobody recorded -------------------------
    let recorded: BTreeSet<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    for img in &files.images {
        if recorded.contains(img.name.as_str()) {
            continue;
        }
        findings.unrecorded_files.push(img.file.clone());
        let remedy = if c.design.is_none() {
            format!(
                "This campaign has no `design.json` at all, so every one of its {n} approved \
                 image(s) is in this state. AUTHOR the document — `delvec schema --stage design` \
                 is its shape — with one row per image saying what it shows and the sky it was \
                 drawn under.",
                n = files.images.len(),
            )
        } else {
            format!(
                "Either (1) AUTHOR its row in `design.json`, with the sky it was drawn under, or \
                 (2) DELETE the file if it was never approved. {n} row(s) present; {f} image \
                 file(s) found.",
                n = rows.len(),
                f = files.images.len(),
            )
        };
        d.push(Diagnostic::error(
            DW_DESIGN_SKY,
            "design",
            format!("design/{}", img.file),
            format!(
                "`design/{file}` is an approved reference image with no row in the design record. \
                 An image under `design/` is a picture somebody said yes to, and the record is \
                 what says which sky they said yes to it in; a file with no row is an approval \
                 this engine cannot hold the world to. {remedy}",
                file = img.file,
            ),
        ));
    }

    // ---- shape a: the two sets of skies are not equal -----------------------
    //
    // Only when the record has rows. With none, there is no art side to
    // compare, and the campaign's state is the measured zero the binding line
    // reports and staging refuses — not a disagreement.
    if !rows.is_empty() {
        let stated_times: Vec<WorldTime> = ordered_times(rows.iter().map(|r| r.time));
        let stated_weathers: Vec<WorldWeather> = ordered_weathers(rows.iter().map(|r| r.weather));
        let time_gap = difference(&world_times, &stated_times);
        let time_extra = difference(&stated_times, &world_times);
        let weather_gap = difference(&world_weathers, &stated_weathers);
        let weather_extra = difference(&stated_weathers, &world_weathers);
        if !time_gap.is_empty()
            || !time_extra.is_empty()
            || !weather_gap.is_empty()
            || !weather_extra.is_empty()
        {
            let mut says = Vec::new();
            if !time_gap.is_empty() {
                says.push(format!(
                    "this world reaches the time(s) {} that no approved picture shows",
                    set_line(&time_gap, WorldTime::keyword)
                ));
            }
            if !time_extra.is_empty() {
                says.push(format!(
                    "the record states the time(s) {} that this world never reaches",
                    set_line(&time_extra, WorldTime::keyword)
                ));
            }
            if !weather_gap.is_empty() {
                says.push(format!(
                    "this world reaches the weather(s) {} that no approved picture shows",
                    set_line(&weather_gap, WorldWeather::keyword)
                ));
            }
            if !weather_extra.is_empty() {
                says.push(format!(
                    "the record states the weather(s) {} that this world never reaches",
                    set_line(&weather_extra, WorldWeather::keyword)
                ));
            }
            d.push(Diagnostic::error(
                DW_DESIGN_SKY,
                "design",
                "/content/references",
                format!(
                    "the approved design and this world do not agree about the sky: {says}. \
                     Measured: world times {wt}, weathers {ww}; stated times {st}, weathers \
                     {sw}, over {n} recorded reference(s). Every hour the party can be in has to \
                     be an hour somebody approved a picture of, and every hour a picture was \
                     approved in has to be one the party can reach — otherwise the delve is lit, \
                     darkness-checked and rendered under a sky the design never chose. Either \
                     (1) DECLARE the hour the record states in `world.json` (`time` and \
                     `weather`), or (2) AUTHOR the design again under the hour this world \
                     reaches and correct the rows in `design.json` to say so. Where the extra \
                     hour comes from a `set-time` or `set-weather` effect, the third move is to \
                     DELETE that effect. Never move the hour to satisfy a mob — that is \
                     `DW0496`'s rule, and it points the other way.",
                    says = says.join("; "),
                    wt = set_line(&world_times, WorldTime::keyword),
                    ww = set_line(&world_weathers, WorldWeather::keyword),
                    st = set_line(&stated_times, WorldTime::keyword),
                    sw = set_line(&stated_weathers, WorldWeather::keyword),
                    n = rows.len(),
                ),
            ));
        }
    }

    (d, binding, findings)
}

/// Read the campaign's showcase camera record and count what it answers of
/// `rows` — through [`crate::compiler::view::camera::tally`], the one comparison
/// between the two documents (spec-0070 §2).
fn camera_record(files: &DesignFiles, rows: &[Reference]) -> CameraRecord {
    use crate::compiler::view::camera;
    let Some(bytes) = &files.cameras else {
        return CameraRecord::Absent;
    };
    match camera::parse_sheet(bytes) {
        Ok(sheet) => {
            let names: Vec<String> = rows.iter().map(|r| r.name.clone()).collect();
            CameraRecord::Read(camera::tally(&sheet, &names))
        }
        Err(d) => CameraRecord::Unreadable(d.message),
    }
}

/// **Every approved picture has a showcase camera** — the build's half of
/// spec-0070, and the record's own rule the build reads it under.
///
/// Two refusals over the same two documents, in the order a creator repairs
/// them: a camera answering a row that does not exist is `DW0721`, the sentence
/// `delvec cameras` has always refused it with, and an approved row no camera
/// answers is `DW0900`. Both are build tier (exit 3), and both are raised
/// **before anything is placed** — the caller runs this after analysis and
/// before `Plan::build`, so the creator's previous build tree is still on disk
/// for `delvec cameras --preview` to draw against.
///
/// An **absent** record raises nothing: the first build of every campaign is the
/// build a camera is written against, so refusing it here would refuse the state
/// every campaign passes through. That zero is measured in the binding line and
/// in `validation/design-record.json`, and refused at the staging gate
/// (`drill3-03`).
pub fn answered(c: &Campaign, files: &DesignFiles) -> Vec<Diagnostic> {
    let rows: &[Reference] = c
        .design
        .as_ref()
        .map(|e| e.content.references.as_slice())
        .unwrap_or(&[]);
    let names: Vec<String> = rows.iter().map(|r| r.name.clone()).collect();
    let CameraRecord::Read(a) = camera_record(files, rows) else {
        // Absent: nothing to hold. Unreadable: `DW0721` is raised by the record's
        // own reader when the build proves the cameras against the world, and a
        // second copy of that refusal here would name the same defect twice.
        return Vec::new();
    };
    let mut d = Vec::new();
    if let Some((name, answers)) = a.stray.first() {
        d.push(Diagnostic::error(
            crate::compiler::view::camera::DW_RECORD_AT_BUILD,
            "design",
            crate::compiler::view::camera::CAMERAS_FILE,
            crate::compiler::view::camera::stray_message(name, answers, &names),
        ));
        // One defect at a time: with a stray camera the record does not yet say
        // which pictures it answers, so counting unanswered rows beside it would
        // report a second number the first defect produced.
        return d;
    }
    if !a.unanswered.is_empty() {
        d.push(Diagnostic::error(
            DW_DESIGN_ANSWERED,
            "design",
            crate::compiler::view::camera::CAMERAS_FILE,
            unanswered_message(&approved_rows(rows), &a),
        ));
    }
    d
}

/// `design.json`'s rows as the camera surface reads them, so the refusal below
/// is built from one shape whichever side raises it.
fn approved_rows(rows: &[Reference]) -> Vec<crate::compiler::view::camera::ApprovedRow> {
    rows.iter()
        .map(|r| crate::compiler::view::camera::ApprovedRow {
            name: r.name.clone(),
            shows: r.shows.clone(),
        })
        .collect()
}

/// The refusal's sentence, shared by the build and by `delvec cameras` so the
/// two give one answer (spec-0070 §5). Each unanswered row is named with its
/// `shows` sentence, because that is what tells a creator which picture it is.
pub fn unanswered_message(
    rows: &[crate::compiler::view::camera::ApprovedRow],
    a: &Answers,
) -> String {
    let shows = |name: &str| -> String {
        rows.iter()
            .find(|r| r.name == name)
            .map(|r| format!(" ({})", r.shows))
            .unwrap_or_default()
    };
    let named = a
        .unanswered
        .iter()
        .map(|n| format!("`{n}`{}", shows(n)))
        .collect::<Vec<_>>()
        .join(", ");
    let answering = if a.answered.is_empty() {
        "no approved image".to_string()
    } else {
        a.answered
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "{} of {} approved image(s) have no showcase camera: {named}. `{file}` holds {} camera(s) \
         answering {answering}. An approved picture nobody has pointed a camera at is a picture \
         the build is never held beside. Either (1) AUTHOR a camera for each: estimate the view \
         from the picture (`showcase-shots.md` §4) and draw it with `delvec cameras <last build> \
         --campaign <dir> -o <dir> --preview`, then write the row (`source: estimated`), or place \
         it by hand in the running game and write it with `delvec place-camera --report`; or (2) \
         DELETE the picture and its row from `design.json`, which is re-opening the design gate \
         for that scene, and is said to the user in those words. Never re-aim an existing camera \
         at a second picture: a row keeps its `answers`, and a camera for another picture is a new \
         row. With no built tree to preview against, (3) DELETE `design/cameras.json`, build, and \
         write it against that build.",
        a.unanswered.len(),
        a.rows(),
        a.cameras,
        file = crate::compiler::view::camera::CAMERAS_FILE,
    )
}

/// The files under one of the two directories, quoted, for a refusal that has
/// to say what IS there.
fn present_list(files: &DesignFiles, dir: &str) -> String {
    let prefix = format!("{dir}/");
    let present: Vec<&str> = files
        .images
        .iter()
        .filter(|i| i.file.starts_with(&prefix))
        .map(|i| i.file.as_str())
        .collect();
    if present.is_empty() {
        "none".to_string()
    } else {
        quoted(&present)
    }
}

/// `` `a`, `b` ``.
fn quoted(items: &[&str]) -> String {
    items
        .iter()
        .map(|f| format!("`{f}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The two vocabularies, in declaration order — the one table this module
/// orders and de-duplicates through.
///
/// Exhaustive both ways, like [`reachable_time_weather`]'s own: a new
/// [`WorldTime`] fails to compile until it is given an index here, so a sky
/// the world can reach can never be silently missing from the comparison.
const TIMES: [WorldTime; 6] = [
    WorldTime::Day,
    WorldTime::Noon,
    WorldTime::Dusk,
    WorldTime::Night,
    WorldTime::Midnight,
    WorldTime::Dawn,
];

/// The weather vocabulary, in declaration order — see [`TIMES`].
const WEATHERS: [WorldWeather; 3] = [
    WorldWeather::Clear,
    WorldWeather::Rain,
    WorldWeather::Thunder,
];

fn time_index(t: WorldTime) -> usize {
    match t {
        WorldTime::Day => 0,
        WorldTime::Noon => 1,
        WorldTime::Dusk => 2,
        WorldTime::Night => 3,
        WorldTime::Midnight => 4,
        WorldTime::Dawn => 5,
    }
}

fn weather_index(w: WorldWeather) -> usize {
    match w {
        WorldWeather::Clear => 0,
        WorldWeather::Rain => 1,
        WorldWeather::Thunder => 2,
    }
}

fn ordered_times(it: impl Iterator<Item = WorldTime>) -> Vec<WorldTime> {
    let set: BTreeSet<usize> = it.map(time_index).collect();
    set.into_iter().map(|i| TIMES[i]).collect()
}

fn ordered_weathers(it: impl Iterator<Item = WorldWeather>) -> Vec<WorldWeather> {
    let set: BTreeSet<usize> = it.map(weather_index).collect();
    set.into_iter().map(|i| WEATHERS[i]).collect()
}

/// The members of `a` that are not in `b`, keeping `a`'s order.
fn difference<T: Copy + PartialEq>(a: &[T], b: &[T]) -> Vec<T> {
    a.iter().copied().filter(|x| !b.contains(x)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_record_prints_its_zeroes() {
        let b = DesignBinding::default();
        let line = b.line();
        assert!(
            line.contains("0 reference(s) recorded over 0 image file(s)"),
            "{line}"
        );
        assert!(line.contains("skies stated: none"), "{line}");
    }

    #[test]
    fn the_binding_line_names_both_directories() {
        let b = DesignBinding {
            references: 4,
            image_files: 4,
            concept_files: 4,
            reference_files: 0,
            skies_stated: vec![(WorldTime::Night, WorldWeather::Clear, 4)],
            world_times: vec![WorldTime::Night],
            world_weathers: vec![WorldWeather::Clear],
            cameras: CameraRecord::Read(Answers {
                cameras: 2,
                answered: vec!["concept/shore-far".to_string()],
                unanswered: vec!["concept/tower-far".to_string()],
                stray: Vec::new(),
            }),
        };
        let line = b.line();
        assert!(
            line.contains("4 reference(s) recorded over 4 image file(s)"),
            "{line}"
        );
        assert!(line.contains("concept/ 4, reference/ 0"), "{line}");
        assert!(line.contains("night+clear x4"), "{line}");
        assert!(
            line.contains("world reaches times {night} weathers {clear}"),
            "{line}"
        );
        assert!(
            line.contains(
                "showcase cameras: 2 in design/cameras.json answering 1 of 4 approved image(s)"
            ),
            "{line}"
        );
    }

    #[test]
    fn a_campaign_with_no_camera_record_says_so_and_counts_its_rows() {
        let b = DesignBinding {
            references: 3,
            ..DesignBinding::default()
        };
        let line = b.line();
        assert!(
            line.contains(
                "showcase cameras: none (no design/cameras.json); 0 of 3 approved image(s) answered"
            ),
            "{line}"
        );
    }
}
