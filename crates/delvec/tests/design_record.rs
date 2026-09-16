//! **The approved hour is the built hour** — `DW0890` and the design record
//! (spec-0061).
//!
//! Every test here drives the real binary over a real campaign directory,
//! because half of what this rule reads is a directory listing and a check that
//! saw only the documents would be blind to exactly the half that says whether
//! the approved pictures are there.
//!
//! The red/green pairs are deliberate: a refusal nobody has watched turn green
//! is a refusal whose remedy nobody has taken.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

mod common;

/// A scratch directory under the test binary's own target tmp — never `/tmp`,
/// which a session or a reboot considers disposable.
fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("design-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn delvec(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(args)
        .output()
        .expect("delvec runs")
}

fn log(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// The smallest thing that counts as an approved image: a file with an image
/// extension. Nothing here opens image bytes, so nothing here needs real ones —
/// which is itself the rule under test (spec-0061 §12).
fn image(camp: &Path, rel: &str) {
    let p = camp.join("design").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, b"not read by anything").unwrap();
}

/// A `design.json` over `rows` of `(name, time, weather)`.
fn record(camp: &Path, rows: &[(&str, &str, &str)]) {
    let refs: Vec<serde_json::Value> = rows
        .iter()
        .map(|(name, time, weather)| {
            serde_json::json!({
                "name": name,
                "shows": "what the picture shows, in one sentence",
                "time": time,
                "weather": weather,
            })
        })
        .collect();
    let doc = serde_json::json!({
        "dsl_version": delvewright_dsl::DSL_VERSION,
        "campaign_id": "hello-world",
        "stage": "design",
        "content": { "references": refs },
    });
    std::fs::write(
        camp.join("design.json"),
        serde_json::to_string_pretty(&doc).unwrap(),
    )
    .unwrap();
}

/// A hello-world copy with its declared hour set to `time`.
fn campaign(tag: &str, time: &str) -> PathBuf {
    let camp = tmp(&format!("camp-{tag}"));
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    common::patch_file(&camp.join("world.json"), |v| {
        v["content"]["time"] = serde_json::json!(time);
    });
    camp
}

fn validate(camp: &Path) -> (i32, String) {
    let r = delvec(&[
        "validate",
        camp.to_str().unwrap(),
        "--prefabs",
        common::prefabs_dir().to_str().unwrap(),
    ]);
    (r.status.code().unwrap_or(-1), log(&r))
}

fn build(tag: &str, camp: &Path) -> (i32, String, PathBuf) {
    let out = tmp(&format!("out-{tag}"));
    let r = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        common::prefabs_dir().to_str().unwrap(),
    ]);
    (r.status.code().unwrap_or(-1), log(&r), out)
}

fn design_record(out: &Path) -> serde_json::Value {
    let p = out.join("validation/design-record.json");
    assert!(
        p.is_file(),
        "every build writes {} — an absent file says `I could not look`, which is a \
         different fact from `references: 0`",
        p.display()
    );
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap()
}

// ---------------------------------------------------------------------------
// Shape a — the world reaches a sky no approved picture shows
// ---------------------------------------------------------------------------

/// **Criterion 3.** Red then green on one campaign, at validation, with nothing
/// placed: the record says `night`, `world.json` says `noon`, and the refusal
/// arrives before a single piece has been seated. Then the move the message
/// names — declare the hour the record states — and the same campaign
/// validates AND builds green.
#[test]
fn a_night_design_under_a_noon_world_is_refused_at_validation_and_the_hour_repairs_it() {
    let red = campaign("shape-a-red", "noon");
    image(&red, "concept/shore-far.png");
    record(&red, &[("concept/shore-far", "night", "clear")]);

    let (code, before) = validate(&red);
    assert_eq!(code, 1, "refused at validation:\n{before}");
    assert!(before.contains("DW0890"), "{before}");
    assert!(
        before.contains("world times {noon}") && before.contains("stated times {night}"),
        "the refusal states both sets:\n{before}"
    );
    // Validation, not the build: nothing is placed to know this.
    assert!(
        !before.contains("site-plan placing:") && !before.contains("blockout sha256"),
        "the verdict is reached with nothing placed:\n{before}"
    );

    let green = campaign("shape-a-green", "night");
    image(&green, "concept/shore-far.png");
    record(&green, &[("concept/shore-far", "night", "clear")]);
    let (code, after) = validate(&green);
    assert_eq!(code, 0, "declaring the hour the record states:\n{after}");
    assert!(!after.contains("DW0890 [error]"), "{after}");
    let (code, built, out) = build("shape-a-green", &green);
    assert_eq!(code, 0, "and it builds:\n{built}");
    assert_eq!(design_record(&out)["references"], 1);
}

/// **Criterion 5, the world-side direction.** The reachable set is read from
/// `compiler::light::reachable_time_weather` and no private scan, so a
/// `set-time` buried inside a `sequence` step — a depth the scan had to be
/// repaired to reach — moves the world's set and fires the same shape.
#[test]
fn a_set_time_inside_a_sequence_step_moves_the_world_side_of_the_comparison() {
    let camp = campaign("sequence-red", "night");
    image(&camp, "concept/shore-far.png");
    record(&camp, &[("concept/shore-far", "night", "clear")]);
    let (code, green) = validate(&camp);
    assert_eq!(code, 0, "the pair agrees to begin with:\n{green}");

    // The effect goes inside a `sequence` step, not at a root: a shallow walk
    // would not see it, and not seeing it is the direction that PASSES.
    common::patch_file(&camp.join("quests.json"), |v| {
        let q = &mut v["content"]["quests"][0];
        let id = q["objectives"][0]["id"].as_str().unwrap().to_string();
        q["on_objective_complete"] = serde_json::json!({
            id: [{
                "type": "sequence",
                "steps": [{ "at_ticks": 0, "effects": [{ "type": "set-time", "time": "dawn" }] }],
            }],
        });
    });
    let (code, red) = validate(&camp);
    assert_eq!(code, 1, "the buried effect is seen:\n{red}");
    assert!(red.contains("DW0890"), "{red}");
    assert!(
        red.contains("reaches the time(s) {dawn} that no approved picture shows"),
        "the refusal names the hour the effect added:\n{red}"
    );

    // The move the message names: DELETE the effect.
    common::patch_file(&camp.join("quests.json"), |v| {
        v["content"]["quests"][0]
            .as_object_mut()
            .unwrap()
            .remove("on_objective_complete");
    });
    let (code, after) = validate(&camp);
    assert_eq!(
        code, 0,
        "removing the effect restores the agreement:\n{after}"
    );
}

// ---------------------------------------------------------------------------
// Shapes b and c — the record and the directory, both ways
// ---------------------------------------------------------------------------

/// **Criterion 4, shape b.** A row that names no file, and a row whose stem two
/// files answer to — a resolve by name over a scope where names are not unique
/// yields a candidate, not a match. Both assert the binding line's two counts.
#[test]
fn a_row_resolves_to_exactly_one_file_or_is_refused() {
    let none = campaign("shape-b-none", "night");
    image(&none, "concept/shore-far.png");
    record(
        &none,
        &[
            ("concept/shore-far", "night", "clear"),
            ("concept/tower-far", "night", "clear"),
        ],
    );
    let (code, out) = validate(&none);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("DW0890"), "{out}");
    assert!(
        out.contains("nothing under `design/concept/` has that stem"),
        "{out}"
    );
    assert!(
        out.contains("files present: `concept/shore-far.png`"),
        "the refusal says what IS there:\n{out}"
    );
    assert!(
        out.contains("2 reference(s) recorded over 1 image file(s)"),
        "the binding line states both counts:\n{out}"
    );

    let two = campaign("shape-b-two", "night");
    image(&two, "concept/tower-far.png");
    image(&two, "concept/tower-far.jpg");
    record(&two, &[("concept/tower-far", "night", "clear")]);
    let (code, out) = validate(&two);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("2 files under `design/concept/` answer to that stem"),
        "{out}"
    );
    assert!(
        out.contains("`concept/tower-far.jpg`") && out.contains("`concept/tower-far.png`"),
        "the candidates are listed:\n{out}"
    );
    assert!(
        out.contains("1 reference(s) recorded over 2 image file(s)"),
        "{out}"
    );
}

/// **Criterion 4, shape c.** A file with no row, and a directory of files with
/// no `design.json` at all — the second is the first over every file, and its
/// remedy names the document and the schema command.
#[test]
fn an_approved_image_nobody_recorded_is_refused() {
    let orphan = campaign("shape-c-orphan", "night");
    image(&orphan, "concept/shore-far.png");
    image(&orphan, "concept/shore-near.png");
    record(&orphan, &[("concept/shore-far", "night", "clear")]);
    let (code, out) = validate(&orphan);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("DW0890"), "{out}");
    assert!(
        out.contains("`design/concept/shore-near.png` is an approved reference image with no row"),
        "{out}"
    );
    assert!(
        out.contains("1 row(s) present; 2 image file(s) found"),
        "{out}"
    );

    let no_doc = campaign("shape-c-no-doc", "night");
    image(&no_doc, "concept/shore-far.png");
    let (code, out) = validate(&no_doc);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("DW0890"), "{out}");
    assert!(
        out.contains("has no `design.json` at all") && out.contains("delvec schema --stage design"),
        "the remedy names the document and its shape:\n{out}"
    );
    assert!(
        out.contains("0 reference(s) recorded over 1 image file(s)"),
        "{out}"
    );
}

/// **Criterion 4, the extension set.** One constant decides what an image is,
/// and a file under `design/` that is not one — the re-issue sidecars live
/// there — is neither counted nor refused.
#[test]
fn a_file_that_is_not_an_image_is_neither_counted_nor_refused() {
    let camp = campaign("sidecar", "night");
    image(&camp, "concept/shore-far.png");
    std::fs::write(
        camp.join("design/concept/shore-far.json"),
        b"{\"prompt\": \"what was asked for, not what was approved\"}",
    )
    .unwrap();
    std::fs::write(camp.join("design/concept/NOTES.md"), b"# notes\n").unwrap();
    record(&camp, &[("concept/shore-far", "night", "clear")]);
    let (code, out) = validate(&camp);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("1 reference(s) recorded over 1 image file(s)"),
        "the sidecar and the notes are not images:\n{out}"
    );
}

// ---------------------------------------------------------------------------
// The artifact, and determinism
// ---------------------------------------------------------------------------

/// **Criterion 7.** `validation/design-record.json` is written by every build,
/// including a build with no record at all — `references: 0` is a number the
/// staging gate reads, and an absent file is a different fact.
#[test]
fn every_build_writes_the_design_record_ledger_including_a_build_with_no_record() {
    let bare = campaign("ledger-bare", "noon");
    let (code, out, tree) = build("ledger-bare", &bare);
    assert_eq!(code, 0, "{out}");
    let r = design_record(&tree);
    assert_eq!(r["references"], 0);
    assert_eq!(r["image_files"], 0);
    assert_eq!(r["by_directory"]["concept"], 0);
    assert_eq!(r["by_directory"]["reference"], 0);
    assert_eq!(r["skies_stated"], serde_json::json!([]));
    assert_eq!(r["world"]["times"], serde_json::json!(["noon"]));
    assert_eq!(r["world"]["weathers"], serde_json::json!(["clear"]));
    assert_eq!(r["unrecorded_files"], serde_json::json!([]));
    assert_eq!(r["unresolved_rows"], serde_json::json!([]));

    let full = campaign("ledger-full", "night");
    image(&full, "concept/shore-far.png");
    image(&full, "reference/whole-map.png");
    record(
        &full,
        &[
            ("concept/shore-far", "night", "clear"),
            ("reference/whole-map", "night", "clear"),
        ],
    );
    let (code, out, tree) = build("ledger-full", &full);
    assert_eq!(code, 0, "{out}");
    let r = design_record(&tree);
    assert_eq!(r["references"], 2);
    assert_eq!(r["image_files"], 2);
    assert_eq!(r["by_directory"]["concept"], 1);
    assert_eq!(r["by_directory"]["reference"], 1);
    assert_eq!(
        r["skies_stated"],
        serde_json::json!([{ "time": "night", "weather": "clear", "count": 2 }])
    );
}

/// **Criterion 10.** Two builds of the same campaign are byte-identical
/// (ADR-0006), the new ledger included.
#[test]
fn two_builds_of_a_campaign_with_a_design_record_are_byte_identical() {
    let camp = campaign("determinism", "night");
    image(&camp, "concept/shore-far.png");
    record(&camp, &[("concept/shore-far", "night", "clear")]);
    let (a_code, a_log, a) = build("determinism-a", &camp);
    let (b_code, b_log, b) = build("determinism-b", &camp);
    assert_eq!(a_code, 0, "{a_log}");
    assert_eq!(b_code, 0, "{b_log}");
    let read = |root: &Path| -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.push((
                        p.strip_prefix(root).unwrap().display().to_string(),
                        std::fs::read(&p).unwrap(),
                    ));
                }
            }
        }
        out.sort();
        out
    };
    let (ra, rb) = (read(&a), read(&b));
    assert!(!ra.is_empty(), "a build with no files is not a build");
    let differing: Vec<&String> = ra
        .iter()
        .zip(rb.iter())
        .filter(|(x, y)| x != y)
        .map(|(x, _)| &x.0)
        .collect();
    assert!(
        ra.len() == rb.len() && differing.is_empty(),
        "two builds of one campaign differ: {differing:?}"
    );
}

// ---------------------------------------------------------------------------
// Shape d — the document's own refusals, which are not `DW0890`
// ---------------------------------------------------------------------------

/// **Shape d** (spec-0061 §6): an empty `references`, a name that is not a path
/// under `design/`, two rows for one picture. Ordinary schema and referential
/// refusals with ordinary codes — listed here so it is provable that they are
/// not a fourth shape of `DW0890`.
#[test]
fn the_records_own_refusals_are_ordinary_and_are_not_dw0890() {
    let empty = campaign("shape-d-empty", "night");
    record(&empty, &[]);
    let (code, out) = validate(&empty);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("DW0100"), "{out}");
    assert!(out.contains("`minItems: 1`"), "{out}");
    assert!(!out.contains("DW0890 [error]"), "{out}");

    let bad_name = campaign("shape-d-name", "night");
    image(&bad_name, "concept/shore-far.png");
    record(&bad_name, &[("sketches/shore-far", "night", "clear")]);
    let (code, out) = validate(&bad_name);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("DW0110"), "{out}");
    assert!(
        out.contains("`concept/<kebab>` or `reference/<kebab>`"),
        "the refusal names the form:\n{out}"
    );

    let dup = campaign("shape-d-duplicate", "night");
    image(&dup, "concept/shore-far.png");
    record(
        &dup,
        &[
            ("concept/shore-far", "night", "clear"),
            ("concept/shore-far", "midnight", "clear"),
        ],
    );
    let (code, out) = validate(&dup);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("DW0111"), "{out}");
    assert!(out.contains("is already used by row 0"), "{out}");
}

/// **The measured zero.** A campaign with no `design/` and no `design.json`
/// prints its zeroes and does not refuse — an optional surface nobody has
/// reached yet is not a defect at this tier, and staging is where that zero is
/// a red.
#[test]
fn a_campaign_with_no_approved_design_states_its_zero_and_passes() {
    let camp = campaign("measured-zero", "noon");
    let (code, out) = validate(&camp);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(
            "design record: 0 reference(s) recorded over 0 image file(s) under `design/` \
             (concept/ 0, reference/ 0); skies stated: none"
        ),
        "the zero is printed, not inferred:\n{out}"
    );
}

// ---------------------------------------------------------------------------
// Every approved image is answered — `DW0900` (spec-0070)
// ---------------------------------------------------------------------------

/// Write `design/cameras.json` over `(name, answers)` pairs, all at `pos`.
///
/// Nothing at validation asks where a camera is; the build does (`DW0724`), so
/// the build cases below take their position from a proven-clear eye.
fn cameras(camp: &Path, pos: [f64; 3], rows: &[(&str, &str)]) {
    let cams: Vec<serde_json::Value> = rows
        .iter()
        .map(|(name, answers)| {
            serde_json::json!({
                "answers": answers, "exposure": 1.0, "fov": 70.0, "height": 90,
                "name": name, "pitch": 10.0, "pos": pos, "source": "estimated",
                "spp": 16, "width": 160, "yaw": 0.0
            })
        })
        .collect();
    std::fs::create_dir_all(camp.join("design")).unwrap();
    std::fs::write(
        camp.join("design/cameras.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "campaign_id": "hello-world", "cameras": cams
        }))
        .unwrap(),
    )
    .unwrap();
}

/// The eye of a hello-world build's first player-POV shot: a point that build
/// itself proved clear, so a showcase camera placed there passes `DW0724`.
fn proven_eye(tag: &str) -> [f64; 3] {
    let camp = campaign(tag, "noon");
    let (code, out, dir) = build(tag, &camp);
    assert_eq!(code, 0, "{out}");
    let plan: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("render-plan.json")).unwrap()).unwrap();
    let pov = plan["shots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kind"] == "pov")
        .expect("hello-world has a walked POV")
        .clone();
    serde_json::from_value(pov["camera"]["pos"].clone()).unwrap()
}

/// **Criterion 1.** The binding line carries the camera count and the answered
/// count on every `validate`, zeroes included, in each state a campaign can be
/// in (spec-0070 §6) — and none of these states is a refusal at validation.
#[test]
fn the_binding_line_states_what_the_cameras_answer_in_every_state() {
    let here = [8.5, 66.0, 8.5];

    // 1. no images, no `design.json`, no record.
    let bare = campaign("answers-bare", "noon");
    let (code, out) = validate(&bare);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(
            "showcase cameras: none (no design/cameras.json); 0 of 0 approved image(s) answered"
        ),
        "{out}"
    );

    // 2. no `design.json`, a record present: it is counted, and its cameras
    //    answer rows that do not exist.
    let orphan = campaign("answers-orphan", "noon");
    cameras(&orphan, here, &[("hero", "concept/shore-far")]);
    let (code, out) = validate(&orphan);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(
            "showcase cameras: 1 in design/cameras.json answering 0 of 0 approved image(s)"
        ) && out
            .contains("1 camera(s) name a row that does not exist (`hero` -> `concept/shore-far`)"),
        "{out}"
    );

    // 3. rows, no record — every campaign between its design step and its first
    //    build.
    let rows_only = campaign("answers-rows", "noon");
    image(&rows_only, "concept/shore-far.png");
    image(&rows_only, "concept/tower-far.png");
    record(
        &rows_only,
        &[
            ("concept/shore-far", "noon", "clear"),
            ("concept/tower-far", "noon", "clear"),
        ],
    );
    let (code, out) = validate(&rows_only);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(
            "showcase cameras: none (no design/cameras.json); 0 of 2 approved image(s) answered"
        ),
        "{out}"
    );

    // 4. rows, a record answering every row.
    let whole = campaign("answers-whole", "noon");
    image(&whole, "concept/shore-far.png");
    image(&whole, "concept/tower-far.png");
    record(
        &whole,
        &[
            ("concept/shore-far", "noon", "clear"),
            ("concept/tower-far", "noon", "clear"),
        ],
    );
    cameras(
        &whole,
        here,
        &[
            ("shore", "concept/shore-far"),
            ("tower", "concept/tower-far"),
        ],
    );
    let (code, out) = validate(&whole);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(
            "showcase cameras: 2 in design/cameras.json answering 2 of 2 approved image(s)"
        ),
        "{out}"
    );

    // 5. rows, a record leaving one unanswered — validation still passes: the
    //    build is where it is refused, so the instruments stay reachable.
    let partial = campaign("answers-partial", "noon");
    image(&partial, "concept/shore-far.png");
    image(&partial, "concept/tower-far.png");
    record(
        &partial,
        &[
            ("concept/shore-far", "noon", "clear"),
            ("concept/tower-far", "noon", "clear"),
        ],
    );
    cameras(&partial, here, &[("shore", "concept/shore-far")]);
    let (code, out) = validate(&partial);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(
            "showcase cameras: 1 in design/cameras.json answering 1 of 2 approved image(s)"
        ),
        "{out}"
    );

    // 6. a camera answering no row, beside rows that exist.
    let stray = campaign("answers-stray", "noon");
    image(&stray, "concept/shore-far.png");
    record(&stray, &[("concept/shore-far", "noon", "clear")]);
    cameras(
        &stray,
        here,
        &[
            ("shore", "concept/shore-far"),
            ("nobody", "concept/a-view-nobody-drew"),
        ],
    );
    let (code, out) = validate(&stray);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(
            "showcase cameras: 2 in design/cameras.json answering 1 of 1 approved image(s)"
        ) && out.contains("(`nobody` -> `concept/a-view-nobody-drew`)"),
        "{out}"
    );
}

/// **Criterion 2.** Red then green at the build: two rows and a record
/// answering one is refused with `DW0900` naming the unanswered row and its
/// `shows` sentence, before any placement line, and the tree a previous build
/// wrote into the same `-o` is byte-unchanged; the second camera builds it green
/// with `answered: 2` in the ledger.
#[test]
fn a_picture_with_no_camera_refuses_the_build_and_the_second_camera_builds_it() {
    let eye = proven_eye("answered-eye");

    let camp = campaign("answered-red", "noon");
    image(&camp, "concept/shore-far.png");
    image(&camp, "concept/tower-far.png");
    record(
        &camp,
        &[
            ("concept/shore-far", "noon", "clear"),
            ("concept/tower-far", "noon", "clear"),
        ],
    );
    cameras(&camp, eye, &[("shore", "concept/shore-far")]);

    // A tree this campaign already built into, before the second picture was
    // approved: what `--preview` draws against, and what the refusal must leave
    // alone.
    let before_camp = campaign("answered-before", "noon");
    image(&before_camp, "concept/shore-far.png");
    record(&before_camp, &[("concept/shore-far", "noon", "clear")]);
    cameras(&before_camp, eye, &[("shore", "concept/shore-far")]);
    let (code, out, tree) = build("answered-before", &before_camp);
    assert_eq!(code, 0, "{out}");
    let manifest = std::fs::read(tree.join("manifest.json")).unwrap();

    let r = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        tree.to_str().unwrap(),
        "--prefabs",
        common::prefabs_dir().to_str().unwrap(),
    ]);
    let refused = log(&r);
    assert_eq!(r.status.code(), Some(3), "{refused}");
    assert!(refused.contains("DW0900"), "{refused}");
    assert!(
        refused.contains("`concept/tower-far` (what the picture shows, in one sentence)"),
        "the refusal names the picture and what it shows:\n{refused}"
    );
    assert!(
        !refused.contains("blockout sha256") && !refused.contains("wrote "),
        "the refusal arrives before anything is placed or written:\n{refused}"
    );
    assert_eq!(
        std::fs::read(tree.join("manifest.json")).unwrap(),
        manifest,
        "a refused build leaves the previous tree alone"
    );

    // Green: the second camera.
    cameras(
        &camp,
        eye,
        &[
            ("shore", "concept/shore-far"),
            ("tower", "concept/tower-far"),
        ],
    );
    let (code, out, dir) = build("answered-green", &camp);
    assert_eq!(code, 0, "{out}");
    let r = design_record(&dir);
    assert_eq!(r["references"], 2);
    assert_eq!(r["cameras"], 2);
    assert_eq!(r["answered"], 2);
    assert_eq!(r["unanswered_rows"], serde_json::json!([]));
}

/// **Criterion 4, the build's half.** With no record the build exits 0 and the
/// ledger reads `cameras: 0`, `answered: 0` and every row unanswered — the
/// measured zero the staging gate refuses and the build cannot.
#[test]
fn a_campaign_with_no_camera_record_builds_and_its_ledger_counts_the_zero() {
    let camp = campaign("answers-ledger", "noon");
    image(&camp, "concept/shore-far.png");
    image(&camp, "concept/tower-far.png");
    record(
        &camp,
        &[
            ("concept/shore-far", "noon", "clear"),
            ("concept/tower-far", "noon", "clear"),
        ],
    );
    let (code, out, dir) = build("answers-ledger", &camp);
    assert_eq!(
        code, 0,
        "a campaign that has not placed a camera builds:\n{out}"
    );
    let r = design_record(&dir);
    assert_eq!(r["references"], 2);
    assert_eq!(r["cameras"], 0);
    assert_eq!(r["answered"], 0);
    assert_eq!(
        r["unanswered_rows"],
        serde_json::json!(["concept/shore-far", "concept/tower-far"]),
        "in the design's own order"
    );

    // And a campaign with no design at all counts its zeroes without inventing
    // an unanswered row.
    let bare = campaign("answers-ledger-bare", "noon");
    let (code, out, dir) = build("answers-ledger-bare", &bare);
    assert_eq!(code, 0, "{out}");
    let r = design_record(&dir);
    assert_eq!(r["cameras"], 0);
    assert_eq!(r["answered"], 0);
    assert_eq!(r["unanswered_rows"], serde_json::json!([]));
}

/// **Criterion 10.** Two builds of a campaign whose record answers every row
/// are byte-identical, the three new ledger keys included (ADR-0006).
#[test]
fn two_builds_of_a_campaign_with_a_camera_record_are_byte_identical() {
    let eye = proven_eye("answered-twice-eye");
    let camp = campaign("answers-determinism", "noon");
    image(&camp, "concept/shore-far.png");
    record(&camp, &[("concept/shore-far", "noon", "clear")]);
    cameras(&camp, eye, &[("shore", "concept/shore-far")]);
    let (a_code, a_log, a) = build("answers-determinism-a", &camp);
    let (b_code, b_log, b) = build("answers-determinism-b", &camp);
    assert_eq!(a_code, 0, "{a_log}");
    assert_eq!(b_code, 0, "{b_log}");
    assert_eq!(
        std::fs::read(a.join("validation/design-record.json")).unwrap(),
        std::fs::read(b.join("validation/design-record.json")).unwrap(),
    );
    assert_eq!(design_record(&a)["answered"], 1);
    assert_eq!(
        std::fs::read(a.join("manifest.json")).unwrap(),
        std::fs::read(b.join("manifest.json")).unwrap(),
    );
}
