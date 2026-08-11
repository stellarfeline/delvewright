//! Contact-sheet tests — the curation page and the rank-never-gate ruling.
//!
//! Neither a GPU nor the client jar is needed: the sheet composites renders that
//! already exist, so these fixtures are PNGs written by the test itself. That is
//! the point of the split — the expensive, EULA-gated, GPU-bound half
//! (`piece`/`batch`) produces the images, and the half that has to be *provably*
//! rank-only runs everywhere, on every push.
//!
//! The load-bearing test is [`low_scoring_candidate_is_present_and_last`]
//! (spec-0028 §5 AC3) and its structural partner
//! [`a_filtering_ranker_cannot_reach_the_page`]: the second is the one that
//! stays red if someone later turns the score into a threshold.

use std::path::{Path, PathBuf};
use std::process::Command;

use delvewright_render::sheet::{self, Candidate, Layout, ScoreSet, SheetOptions};

const BIN: &str = env!("CARGO_BIN_EXE_delve-render");

fn tmp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delve-sheet-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write a solid-color PNG. Deliberately non-square so the aspect-preserving
/// thumbnail path is exercised rather than assumed.
fn png(path: &Path, rgb: [u8; 3]) {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    let img = image::RgbaImage::from_pixel(64, 48, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
    img.save(path).unwrap();
}

/// A `delve-render batch`-shaped tree: one directory of shots per candidate.
fn batch_tree(root: &Path, ids: &[&str]) {
    for (i, id) in ids.iter().enumerate() {
        let shade = (40 + i * 30) as u8;
        png(
            &root.join(id).join(format!("{id}-ext-se.png")),
            [shade, 60, 90],
        );
        png(
            &root.join(id).join(format!("{id}-top.png")),
            [90, shade, 60],
        );
    }
}

fn scores_file(path: &Path, backend: &str, higher: bool, rows: &[(&str, f64)]) {
    let scores: Vec<serde_json::Value> = rows
        .iter()
        .map(|(id, s)| serde_json::json!({ "id": id, "image": format!("{id}.png"), "score": s }))
        .collect();
    let doc = serde_json::json!({
        "schema": "delvewright.refscore/1",
        "backend": backend,
        "model": "test",
        "reference": "ref.png",
        "higher_is_better": higher,
        "rank_only_never_gates": true,
        "scores": scores,
    });
    std::fs::write(path, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
}

fn manifest(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn ranked_ids(m: &serde_json::Value) -> Vec<String> {
    m["cells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect()
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(BIN).args(args).output().unwrap()
}

// ---------------------------------------------------------------------------
// The ruling: the score RANKS, it never GATES
// ---------------------------------------------------------------------------

/// spec-0028 §5 AC3, end to end through the binary: the worst-scoring candidate
/// is **still present, last**.
#[test]
fn low_scoring_candidate_is_present_and_last() {
    let dir = tmp("ac3");
    let renders = dir.join("renders");
    batch_tree(&renders, &["gatehouse", "hovel", "shrine", "temple"]);
    let scores = dir.join("scores.json");
    // `hovel` is the low scorer — an order of magnitude below the rest.
    scores_file(
        &scores,
        "stub",
        true,
        &[
            ("temple", 0.91),
            ("shrine", 0.55),
            ("gatehouse", 0.40),
            ("hovel", 0.01),
        ],
    );
    let out = dir.join("sheet.png");

    let r = run(&[
        "contact-sheet",
        renders.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--scores",
        scores.to_str().unwrap(),
    ]);
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    assert!(out.exists(), "the page was not written");

    let m = manifest(&dir.join("sheet.json"));
    let ids = ranked_ids(&m);
    assert_eq!(
        ids,
        ["temple", "shrine", "gatehouse", "hovel"],
        "the page must be ordered by score"
    );
    assert!(
        ids.contains(&"hovel".to_string()),
        "the low scorer is PRESENT"
    );
    assert_eq!(ids.last().unwrap(), "hovel", "the low scorer is LAST");
    assert_eq!(
        m["cells"].as_array().unwrap().len(),
        4,
        "no cell was dropped"
    );
    assert_eq!(m["rank_only_never_gates"], serde_json::json!(true));

    // Binding count, stated on the artifact and on stderr — a gate that binds
    // to nothing is vacuous, not a pass.
    assert_eq!(m["binding"]["candidates"], serde_json::json!(4));
    assert_eq!(m["binding"]["scored"], serde_json::json!(4));
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(stderr.contains("4 candidate(s), 4 scored"), "{stderr}");
}

/// The structural guard, exercised through the composition entry point in the
/// exact direction this ruling erodes: a ranker that applies a threshold.
///
/// This test is why the ordering is a *seam*. Adding `.filter(|c| score > t)`
/// anywhere in the ranking cannot ship — it lands here as `DW0725` — so
/// promoting the score to a gate is a deliberate act against a red check, not
/// an incremental refactor nobody notices.
#[test]
fn a_filtering_ranker_cannot_reach_the_page() {
    let dir = tmp("gate-guard");
    let renders = dir.join("renders");
    batch_tree(&renders, &["a", "b", "c"]);
    let (candidates, layout) = sheet::discover(&renders, None).unwrap();
    assert_eq!(candidates.len(), 3);
    assert_eq!(layout, Layout::PerCandidateDir);

    // Exactly what a threshold does: it returns fewer indices than it was given.
    let gating = |c: &[Candidate], _: Option<&ScoreSet>| (0..c.len()).skip(1).collect::<Vec<_>>();
    let err = sheet::build_sheet(
        &renders,
        &candidates,
        None,
        layout,
        None,
        &SheetOptions::default(),
        gating,
    )
    .unwrap_err();
    assert_eq!(err.code, "DW0725");
    assert!(err.message.contains("NEVER gates"), "{}", err.message);

    // And the honest ranker passes the same guard.
    sheet::build_sheet(
        &renders,
        &candidates,
        None,
        layout,
        None,
        &SheetOptions::default(),
        sheet::rank_by_score,
    )
    .expect("rank-only ordering must build");
}

/// An unscored candidate is a *missing measurement*, not a bad one: it stays on
/// the page, last, and is named as unscored — never quietly dropped and never
/// silently mixed in among the measured ones.
#[test]
fn unscored_candidates_stay_on_the_page_and_the_binding_is_reported() {
    let dir = tmp("partial-binding");
    let renders = dir.join("renders");
    batch_tree(&renders, &["keep", "measured", "zulu"]);
    let scores = dir.join("scores.json");
    scores_file(&scores, "stub", true, &[("measured", 0.7), ("ghost", 0.9)]);
    let out = dir.join("sheet.png");

    let r = run(&[
        "contact-sheet",
        renders.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--scores",
        scores.to_str().unwrap(),
    ]);
    assert_eq!(
        r.status.code(),
        Some(0),
        "a partial binding is a warning, not a stop"
    );
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(stderr.contains("DW0726"), "expected DW0726: {stderr}");
    assert!(stderr.contains("1 of 3"), "{stderr}");
    assert!(stderr.contains("matched no candidate"), "{stderr}");

    let m = manifest(&dir.join("sheet.json"));
    assert_eq!(ranked_ids(&m), ["measured", "keep", "zulu"]);
    assert_eq!(m["binding"]["scored"], serde_json::json!(1));
    assert_eq!(m["binding"]["candidates"], serde_json::json!(3));
    assert_eq!(
        m["binding"]["unmatched_score_rows"],
        serde_json::json!(["ghost"])
    );
}

/// A score set that bound to ZERO candidates ordered nothing at all, and must
/// not read as a successful ranking run.
#[test]
fn zero_binding_is_dw0726_exit2() {
    let dir = tmp("zero-binding");
    let renders = dir.join("renders");
    batch_tree(&renders, &["a", "b"]);
    let scores = dir.join("scores.json");
    scores_file(&scores, "stub", true, &[("x", 0.5), ("y", 0.4)]);

    let r = run(&[
        "contact-sheet",
        renders.to_str().unwrap(),
        "-o",
        dir.join("sheet.png").to_str().unwrap(),
        "--scores",
        scores.to_str().unwrap(),
    ]);
    assert_eq!(r.status.code(), Some(2), "{r:?}");
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(stderr.contains("DW0726"), "{stderr}");
    assert!(stderr.contains("bound to 0 of 2"), "{stderr}");
    assert!(!dir.join("sheet.png").exists(), "no page on a zero binding");
}

// ---------------------------------------------------------------------------
// The page itself
// ---------------------------------------------------------------------------

#[test]
fn without_scores_the_page_is_id_order_and_says_so() {
    let dir = tmp("unranked");
    let renders = dir.join("renders");
    batch_tree(&renders, &["zulu", "alpha", "mike"]);
    let out = dir.join("sheet.png");

    let r = run(&[
        "contact-sheet",
        renders.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let m = manifest(&dir.join("sheet.json"));
    assert_eq!(ranked_ids(&m), ["alpha", "mike", "zulu"]);
    assert!(m["rank_source"].is_null(), "no score file, no rank source");
    assert_eq!(m["layout"], serde_json::json!("per-candidate-dir"));
    // Every cell is placed: 3 candidates over ceil(sqrt(3)) = 2 columns.
    assert_eq!(m["columns"], serde_json::json!(2));
    let cells = m["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 3);
    assert_eq!(cells[2]["row"], serde_json::json!(1));
    assert_eq!(cells[2]["col"], serde_json::json!(0));
}

/// Two runs over the same inputs produce the same page, byte for byte. Sheets
/// are working artifacts (outside ADR-0006 like every render), but a curation
/// page that shifted between runs would make "the owner picked cell 7" mean
/// two different things.
#[test]
fn the_page_is_reproducible_byte_for_byte() {
    let dir = tmp("determinism");
    let renders = dir.join("renders");
    batch_tree(&renders, &["a", "b", "c", "d", "e"]);
    let scores = dir.join("scores.json");
    scores_file(
        &scores,
        "stub",
        true,
        &[("a", 0.1), ("b", 0.9), ("c", 0.5), ("d", 0.5), ("e", 0.2)],
    );

    let mut pages = Vec::new();
    for pass in ["one", "two"] {
        let out = dir.join(format!("{pass}.png"));
        let r = run(&[
            "contact-sheet",
            renders.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--scores",
            scores.to_str().unwrap(),
            "--title",
            "fixed title",
        ]);
        assert_eq!(r.status.code(), Some(0), "{r:?}");
        pages.push((
            std::fs::read(&out).unwrap(),
            ranked_ids(&manifest(&dir.join(format!("{pass}.json")))),
        ));
    }
    assert_eq!(pages[0].0, pages[1].0, "the composited page must be stable");
    assert_eq!(pages[0].1, pages[1].1);
    // The tie between c and d resolves by id, so the order is total.
    assert_eq!(pages[0].1, ["b", "c", "d", "e", "a"]);
}

#[test]
fn an_explicit_shot_is_never_silently_substituted() {
    let dir = tmp("shot-refused");
    let renders = dir.join("renders");
    batch_tree(&renders, &["a", "b"]);
    // `b` has no `door-0`; asking for one must be an error, not another angle.
    let r = run(&[
        "contact-sheet",
        renders.to_str().unwrap(),
        "-o",
        dir.join("sheet.png").to_str().unwrap(),
        "--shot",
        "door-0",
    ]);
    assert_eq!(r.status.code(), Some(2), "{r:?}");
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(stderr.contains("DW0721"), "{stderr}");
    assert!(stderr.contains("not a comparison"), "{stderr}");
}

#[test]
fn a_named_shot_selects_that_angle_for_every_cell() {
    let dir = tmp("shot-named");
    let renders = dir.join("renders");
    batch_tree(&renders, &["a", "b"]);
    let r = run(&[
        "contact-sheet",
        renders.to_str().unwrap(),
        "-o",
        dir.join("sheet.png").to_str().unwrap(),
        "--shot",
        "top",
    ]);
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let m = manifest(&dir.join("sheet.json"));
    assert_eq!(m["shot"], serde_json::json!("top"));
    for cell in m["cells"].as_array().unwrap() {
        assert!(
            cell["image"].as_str().unwrap().ends_with("-top.png"),
            "{cell}"
        );
    }
}

#[test]
fn a_flat_directory_of_renders_is_a_sheet_too() {
    let dir = tmp("flat");
    let renders = dir.join("renders");
    png(&renders.join("second.png"), [10, 20, 30]);
    png(&renders.join("first.png"), [30, 20, 10]);
    let r = run(&[
        "contact-sheet",
        renders.to_str().unwrap(),
        "-o",
        dir.join("sheet.png").to_str().unwrap(),
    ]);
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let m = manifest(&dir.join("sheet.json"));
    assert_eq!(m["layout"], serde_json::json!("flat"));
    assert_eq!(ranked_ids(&m), ["first", "second"]);
}

#[test]
fn an_empty_directory_is_dw0721_exit2() {
    let dir = tmp("empty");
    let renders = dir.join("renders");
    std::fs::create_dir_all(&renders).unwrap();
    let r = run(&[
        "contact-sheet",
        renders.to_str().unwrap(),
        "-o",
        dir.join("sheet.png").to_str().unwrap(),
    ]);
    assert_eq!(r.status.code(), Some(2), "{r:?}");
    assert!(String::from_utf8_lossy(&r.stderr).contains("DW0721"));
}

#[test]
fn a_malformed_score_file_is_refused_before_any_page_is_drawn() {
    let dir = tmp("bad-scores");
    let renders = dir.join("renders");
    batch_tree(&renders, &["a"]);
    let scores = dir.join("scores.json");
    // No `higher_is_better`: the sort direction is never guessed.
    std::fs::write(
        &scores,
        br#"{"schema":"delvewright.refscore/1","backend":"stub","scores":[]}"#,
    )
    .unwrap();
    let r = run(&[
        "contact-sheet",
        renders.to_str().unwrap(),
        "-o",
        dir.join("sheet.png").to_str().unwrap(),
        "--scores",
        scores.to_str().unwrap(),
    ]);
    assert_eq!(r.status.code(), Some(2), "{r:?}");
    assert!(String::from_utf8_lossy(&r.stderr).contains("higher_is_better"));
    assert!(!dir.join("sheet.png").exists());
}

// ---------------------------------------------------------------------------
// The whole loop, with the stub model — spec-0028 §5 AC2
// ---------------------------------------------------------------------------

/// sheet → `tools/refscore.py --backend stub` → ranked sheet, offline, no key,
/// no model, no network.
///
/// This is also the gate that keeps the two halves speaking the same language:
/// `delve-render` is the only discoverer of candidates (the scorer reads the
/// sheet manifest), so **every** candidate must bind. A zero or partial binding
/// here is the drift this test exists to catch, which is why it asserts the
/// count rather than merely asserting exit 0.
#[test]
fn the_stub_loop_scores_and_reorders_the_page_offline() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let refscore = repo.join("tools/refscore.py");
    assert!(refscore.exists(), "{} is missing", refscore.display());

    let dir = tmp("stub-loop");
    let renders = dir.join("renders");
    batch_tree(&renders, &["alpha", "bravo", "charlie", "delta"]);
    let reference = dir.join("reference.png");
    png(&reference, [200, 180, 140]);
    let sheet_png = dir.join("sheet.png");
    let sheet_json = dir.join("sheet.json");

    // 1. the unranked page — it always writes the manifest naming every cell
    let r = run(&[
        "contact-sheet",
        renders.to_str().unwrap(),
        "-o",
        sheet_png.to_str().unwrap(),
    ]);
    assert_eq!(r.status.code(), Some(0), "{r:?}");

    // 2. score those exact candidates with the stub backend
    let scores = dir.join("scores.json");
    let py = Command::new("python3")
        .arg(&refscore)
        .args(["--sheet", sheet_json.to_str().unwrap()])
        .args(["--reference", reference.to_str().unwrap()])
        .args(["--backend", "stub"])
        .args(["-o", scores.to_str().unwrap()])
        .output()
        .expect("python3 is required to run the scorer half of the loop");
    assert!(
        py.status.success(),
        "refscore.py failed: {}",
        String::from_utf8_lossy(&py.stderr)
    );
    let doc: serde_json::Value = serde_json::from_slice(&std::fs::read(&scores).unwrap()).unwrap();
    assert_eq!(doc["backend"], serde_json::json!("stub"));
    assert_eq!(doc["higher_is_better"], serde_json::json!(true));
    assert_eq!(doc["scores"].as_array().unwrap().len(), 4);
    assert!(
        doc["note"]
            .as_str()
            .unwrap()
            .contains("NOT a similarity measure"),
        "a stub score must announce itself on the artifact: {}",
        doc["note"]
    );

    // 3. the same page, now ordered by the score — and every candidate bound
    let r = run(&[
        "contact-sheet",
        renders.to_str().unwrap(),
        "-o",
        sheet_png.to_str().unwrap(),
        "--scores",
        scores.to_str().unwrap(),
    ]);
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let m = manifest(&sheet_json);
    assert_eq!(
        m["binding"]["scored"], m["binding"]["candidates"],
        "the scorer and the sheet must agree on every candidate id — binding: {}",
        m["binding"]
    );
    assert_eq!(m["binding"]["scored"], serde_json::json!(4));
    assert_eq!(m["rank_source"]["backend"], serde_json::json!("stub"));

    // The page really is in score order, and still holds all four.
    let by_score = {
        let mut rows: Vec<(String, f64)> = doc["scores"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["id"].as_str().unwrap().to_string(),
                    r["score"].as_f64().unwrap(),
                )
            })
            .collect();
        rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        rows.into_iter().map(|(id, _)| id).collect::<Vec<_>>()
    };
    assert_eq!(ranked_ids(&m), by_score);
    assert_eq!(ranked_ids(&m).len(), 4);
}

/// `--dry-run` measures nothing and writes nothing: the harness half of
/// spec-0028 §5 AC2.
#[test]
fn the_scorer_dry_run_writes_nothing() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let dir = tmp("dry-run");
    let renders = dir.join("renders");
    batch_tree(&renders, &["a", "b"]);
    let reference = dir.join("reference.png");
    png(&reference, [1, 2, 3]);
    let sheet_png = dir.join("sheet.png");
    assert_eq!(
        run(&[
            "contact-sheet",
            renders.to_str().unwrap(),
            "-o",
            sheet_png.to_str().unwrap()
        ])
        .status
        .code(),
        Some(0)
    );
    let out = dir.join("nothing.json");
    let py = Command::new("python3")
        .arg(repo.join("tools/refscore.py"))
        .args(["--sheet", dir.join("sheet.json").to_str().unwrap()])
        .args(["--reference", reference.to_str().unwrap()])
        .args(["--backend", "stub", "--dry-run"])
        .args(["-o", out.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        py.status.success(),
        "{}",
        String::from_utf8_lossy(&py.stderr)
    );
    assert!(!out.exists(), "--dry-run must write nothing");
    let stdout = String::from_utf8_lossy(&py.stdout);
    assert!(stdout.contains("candidates : 2"), "{stdout}");
    assert!(stdout.contains("never gates"), "{stdout}");
}
